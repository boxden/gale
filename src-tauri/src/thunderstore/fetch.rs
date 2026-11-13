use core::str;
use std::{
    io::Read,
    sync::LazyLock,
    time::{Duration, Instant},
};

use eyre::{Report, Result};
use flate2::read::GzDecoder;
use futures_util::{StreamExt, TryFutureExt, future, stream};
use http_cache_reqwest::CacheMode;
use indexmap::IndexMap;
use serde::Serialize;
use tauri::AppHandle;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};

use crate::{
    game::Game,
    logger,
    state::ManagerExt,
    thunderstore::{Backend, PackageListing},
};

pub async fn fetch_package_loop(game: Game, app: AppHandle, cancel_token: CancellationToken) {
    let backends = app.lock_prefs().enabled_backends(game);
    future::join_all(
        backends
            .iter()
            .map(|backend| fetch_single_package_loop(game, app.clone(), backend, &cancel_token)),
    )
    .await;
}

async fn fetch_single_package_loop(
    game: Game,
    app: AppHandle,
    backend: Backend,
    cancel_token: &CancellationToken,
) {
    const FETCH_INTERVAL: Duration = Duration::from_mins(15);

    let mut is_first = true;

    loop {
        let fetch_automatically = app.lock_prefs().fetch_mods_automatically;

        // always fetch once, even if the setting is turned off
        if !fetch_automatically && !is_first {
            info!("automatic fetch cancelled by user setting");
            break;
        }

        if let Err(err) = loop_iter(game, &mut is_first, &app, backend, cancel_token).await {
            logger::log_webview_err(
                format!("Error while fetching packages from {backend:?}"),
                err,
                &app,
            );
        }

        tokio::select! {
            () = cancel_token.cancelled() => {
                debug!("fetch loop cancelled while waiting for next iteration");
                break;
            }
            () = tokio::time::sleep(FETCH_INTERVAL) => {}
        }
    }

    async fn loop_iter(
        game: Game,
        is_first: &mut bool,
        app: &AppHandle,
        backend: Backend,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if app.lock_thunderstore().is_fetching {
            warn!("automatic fetch cancelled due to ongoing fetch");
            return Ok(());
        }

        let result = fetch_single_packages(game, *is_first, app, backend, cancel_token).await;

        let mut state = app.lock_thunderstore();
        state.is_fetching = false;

        let backend_state = state.backend_mut(backend);
        backend_state.packages_fetched |= result.is_ok();

        *is_first &= result.is_err();

        // notify frontend to show any mod updates that were just fetched
        app.lock_manager().active_profile().notify_frontend(app)?;

        result
    }
}

const EXCLUDED_PACKAGES_STR: &str = include_str!("../../excluded_packages.txt");

static EXCLUDED_PACKAGES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| EXCLUDED_PACKAGES_STR.split('\n').map(str::trim).collect());

pub(super) async fn fetch_packages(
    game: Game,
    write_directly: bool,
    app: &AppHandle,
    cancel_token: &CancellationToken,
) -> Vec<(Backend, Report)> {
    let backends = app.lock_prefs().enabled_backends(game);
    let result = future::join_all(backends.iter().map(|backend| {
        fetch_single_packages(game, write_directly, app, backend, cancel_token)
            .map_err(move |err| (backend, err))
    }))
    .await
    .into_iter()
    .filter_map(Result::err)
    .collect();

    let mut state = app.lock_thunderstore();
    state.is_fetching = false;

    result
}

async fn fetch_single_packages(
    game: Game,
    write_directly: bool,
    app: &AppHandle,
    backend: Backend,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let start_time = Instant::now();

    let Some(index_url) = backend.index_url(game) else {
        app.lock_thunderstore()
            .backend_mut(backend)
            .packages_fetched = true;
        return Ok(());
    };

    emit_event(FetchEvent::Start { backend }, app);

    let result = try_fetch(index_url, write_directly, app, backend, game, cancel_token).await;

    emit_event(FetchEvent::Done { backend }, app);

    match result {
        Ok(count) => {
            debug!(
                "fetched {} {:?} packages for {} in {:?}",
                count,
                backend,
                game.slug,
                start_time.elapsed()
            );

            return Ok(());
        }
        Err(err) => return Err(err),
    }

    async fn try_fetch(
        index_url: String,
        write_directly: bool,
        app: &AppHandle,
        backend: Backend,
        game: Game,
        cancel_token: &CancellationToken,
    ) -> Result<usize> {
        let bytes = tokio::select! {
            biased;
            () = cancel_token.cancelled() => {
                debug!("fetch cancelled while fetching packages for {}", backend);
                return Ok(0);
            }
            res = async {
                // the index must never be served from the cache: it 302-redirects
                // to an immutable blob whose 31-day max-age would otherwise be
                // stored under the index URL, freezing the catalog for a month
                let response = app
                    .http()
                    .get(&index_url)
                    .with_extension(CacheMode::NoStore)
                    .send()
                    .await?;
                let bytes = response.error_for_status()?.bytes().await?;

                Ok::<_, reqwest_middleware::Error>(bytes)
            } => res?,
        };

        let urls: Vec<String> = serde_json::from_reader(GzDecoder::new(&bytes[..]))?;

        // The index lists content-addressed chunks, so an unchanged index means
        // the package map already has exactly these packages; re-parsing the
        // whole catalog would only reproduce it.
        if !write_directly {
            let state = app.lock_thunderstore();

            if state.backend(backend).last_index.as_deref() == Some(urls.as_slice()) {
                debug!("package index for {} is unchanged, skipping fetch", backend);
                return Ok(state.backend(backend).packages.len());
            }
        }

        let mut package_count = 0;
        let mut package_buffer = IndexMap::new();

        let (tx, mut rx) = mpsc::channel(4);

        let handle = app.to_owned();
        let chunk_urls = urls.clone();
        let chunk_fetcher = tokio::spawn(async move {
            if let Err(err) = fetch_chunks(tx, chunk_urls, handle).await {
                error!("failed to request package listing chunks: {err:#}");
            }
        });

        loop {
            tokio::select! {
                biased;
                () = cancel_token.cancelled() => {
                    debug!("fetch cancelled while fetching packages for {}", backend);
                    chunk_fetcher.abort();
                    return Ok(0);
                }
                chunk = rx.recv() => {
                    let Some(chunk) = chunk else {
                        break;
                    };

                    let packages = chunk?;

                    let packages = packages
                        .into_iter()
                        .filter(|package| {
                            !EXCLUDED_PACKAGES
                                .iter()
                                .any(|excluded| package.full_name() == *excluded)
                        })
                        .map(|package| (package.uuid, PackageListing { backend, ..package }));

                    let prev_package_count = package_count;

                    if write_directly {
                        let mut state = app.lock_thunderstore();

                        if state.game != Some(game) {
                            debug!("stopping fetch because the active game has changed");
                            return Ok(0);
                        }

                        let backend_state = state.backend_mut(backend);
                        let prev_count = backend_state.packages.len();
                        backend_state.packages.extend(packages);

                        package_count += backend_state.packages.len() - prev_count;
                    } else {
                        package_buffer.extend(packages);

                        package_count = package_buffer.len();
                    }

                    let batch_size = package_count - prev_package_count;

                    trace!(len = batch_size, index_url, "received package batch");

                    emit_event(
                        FetchEvent::Progress {
                            backend,
                            mods: batch_size,
                        },
                        app,
                    );
                }
            }
        }

        let mut state = app.lock_thunderstore();

        if state.game != Some(game) {
            debug!("stopping fetch because the active game has changed");
            return Ok(0);
        }

        let backend_state = state.backend_mut(backend);
        backend_state.packages_fetched = true;
        backend_state.last_index = Some(urls);

        if !write_directly {
            backend_state.packages = package_buffer;
        }

        chunk_fetcher.abort();

        Ok(backend_state.packages.len())
    }

    async fn fetch_chunks(
        tx: mpsc::Sender<Result<Vec<PackageListing>>>,
        urls: Vec<String>,
        app: AppHandle,
    ) -> Result<()> {
        let mut chunks = stream::iter(urls)
            .map(|url| {
                let app = app.clone();
                async move {
                    // chunk URLs contain the content hash, so cached copies
                    // are immutable and can be reused across launches
                    let bytes = app
                        .http()
                        .get(url)
                        .with_extension(CacheMode::ForceCache)
                        .send()
                        .await?
                        .bytes()
                        .await?;

                    // parsing a chunk takes hundreds of milliseconds, so do it
                    // on a blocking thread and let the other chunks download
                    Ok::<_, eyre::Report>(
                        tokio::task::spawn_blocking(move || -> Result<Vec<PackageListing>> {
                            let mut text = String::new();
                            GzDecoder::new(&bytes[..]).read_to_string(&mut text)?;
                            Ok(serde_json::from_str(&text)?)
                        })
                        .await??,
                    )
                }
            })
            .buffer_unordered(4);

        while let Some(chunk) = chunks.next().await {
            if tx.send(chunk).await.is_err() {
                debug!("chunk channel closed early, stopping package fetch");
                break;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "type")]
enum FetchEvent {
    Start { backend: Backend },
    Progress { backend: Backend, mods: usize },
    Done { backend: Backend },
}

fn emit_event(event: FetchEvent, app: &AppHandle) {
    app.emit_buffered("fetch_event", &event);
}

pub async fn wait_for_fetch(app: &AppHandle) {
    loop {
        let game = app.lock_manager().active_game;

        if app.lock_thunderstore().packages_fetched(app, game) {
            return;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
