<script module lang="ts">
	import type { BrowsedMod } from '$lib/types';

	// result set cached at module level: client-side navigation remounts
	// this page, and starting from an empty array meant the list stayed
	// blank for the whole duration of the (potentially large) query.
	// Rendering the previous results instantly lets the scroll restore
	// fire immediately while refresh() updates the data in the background.
	let cachedMods: BrowsedMod[] = [];
	let cachedGame: string | null = null;
	let cachedSelectedKey: string | null = null;
</script>

<script lang="ts">
	import * as api from '$lib/api';
	import { type SortBy, type Mod, type ModId, type ModContextItem } from '$lib/types';

	import ModList from '$lib/components/mod-list/ModList.svelte';

	import { onDestroy, onMount, untrack } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import ModListItem from '$lib/components/mod-list/ModListItem.svelte';
	import ProfileLockedBanner from '$lib/components/mod-list/ProfileLockedBanner.svelte';
	import ModListFilters from '$lib/components/mod-list/ModListFilters.svelte';
	import InstallModButton from '$lib/components/mod-list/InstallModButton.svelte';
	import { getDefaultContextItems } from '$lib/context';
	import profiles from '$lib/state/profile.svelte';
	import { modQuery } from '$lib/state/misc.svelte';
	import installState from '$lib/state/install.svelte';
	import games from '$lib/state/game.svelte';
	import { scrollAnchors } from '$lib/scrollMemory';
	import { m } from '$lib/paraglide/messages';
	import { pushInfoToast, pushToast } from '$lib/toast';
	import HelpCard from '$lib/components/ui/HelpCard.svelte';
	import ForeignDownloadDialog from '$lib/components/dialogs/ForeignDownloadDialog.svelte';
	import {
		extractDeduplicatedMod,
		getPreferredBackend,
		shouldWarnForeignDownload
	} from '$lib/util';
	import DeduplicatedModDetails from '$lib/components/mod-list/DeduplicatedModDetails.svelte';

	const sortOptions: SortBy[] = ['lastUpdated', 'newest', 'rating', 'downloads'];
	const contextItems = $derived<ModContextItem[]>([
		{
			label: m.browse_contextItem_hideMod(),
			icon: 'mdi:eye-off',
			onclick: async (mod: Mod) => {
				await api.profile.toggleHiddenMod(mod.uuid);
				await refresh();
				pushInfoToast({
					message: m.browse_contextitem_hideMod_message({ name: mod.name })
				});
			}
		},
		...getDefaultContextItems()
	]);

	const initialGame = untrack(() => games.active?.slug ?? null);
	const scrollKey = $derived(`browse-mods:${games.active?.slug ?? 'none'}`);

	if (cachedGame !== initialGame) cachedSelectedKey = null;

	let mods: BrowsedMod[] = $state(cachedGame === initialGame ? cachedMods : []);
	// prefetch enough mods for the remembered anchor index to exist
	let maxCount: number = $state(maxInitialCount(initialGame));
	let selectedMod: BrowsedMod | null = $state(
		cachedSelectedKey ? findItemByUuid(cachedSelectedKey) : null
	);
	let lastGame = initialGame;

	function maxInitialCount(game: string | null) {
		return Math.max(20, (scrollAnchors.get(`browse-mods:${game ?? 'none'}`)?.index ?? 0) + 80);
	}

	$effect.pre(() => {
		const game = games.active?.slug ?? null;
		if (game === lastGame) return;
		lastGame = game;
		mods = cachedGame === game ? cachedMods : [];
		maxCount = maxInitialCount(game);
		selectedMod = null;
		cachedSelectedKey = null;
	});
	let foreignDownloadDialogOpen = $state(false);

	let installId: ModId;
	let unlistenFromQuery: UnlistenFn | undefined;

	function cacheMods(newMods: BrowsedMod[], game: string | null) {
		cachedMods = newMods;
		cachedGame = game;

		// a response for a game that is no longer active must not paint over
		// the current catalog; its cache entry stays valid for the next visit
		if (game === (games.active?.slug ?? null)) mods = newMods;
	}

	onMount(() => {
		listen<BrowsedMod[]>('mod_query_result', (evt) => {
			cacheMods(evt.payload, games.active?.slug ?? null);
		}).then((unlisten) => {
			unlistenFromQuery = unlisten;
		});

		return () => {
			unlistenFromQuery?.();
			api.thunderstore.stopQuerying();
		};
	});

	let hasRefreshed = $state(false);
	let refreshing = false;
	let refreshPending = false;

	function itemUuid(item: BrowsedMod) {
		return item.data.thunderstore?.uuid ?? item.data.hexium?.uuid;
	}

	function findItemByUuid(uuid: string | undefined) {
		return mods.find((mod) => itemUuid(mod) === uuid) ?? null;
	}

	async function refresh() {
		if (refreshing) {
			refreshPending = true;
			return;
		}

		refreshing = true;
		try {
			do {
				refreshPending = false;

				const game = games.active?.slug ?? null;

				cacheMods(await api.thunderstore.query({ ...modQuery.current, maxCount }), game);
				if (cachedSelectedKey) {
					const found = findItemByUuid(cachedSelectedKey);

					if (found) {
						// isInstalled might have changed
						selectedMod = found;
					} else if (selectedMod) {
						selectedMod = null;
						cachedSelectedKey = null;
					}
				}
			} while (refreshPending);

			hasRefreshed = true;
		} catch (err) {
			console.error('failed to refresh mod catalog', err);
			pushToast({
				type: 'error',
				name: 'Browse',
				message: 'Failed to load the mod catalog. Check the log for details.'
			});
			hasRefreshed = true;
		} finally {
			refreshing = false;
		}
	}

	async function installLatest(mod: Mod) {
		await install({
			packageUuid: mod.uuid,
			versionUuid: mod.versions[0].uuid,
			backend: mod.backend
		});
	}

	async function doInstall() {
		if (locked) return;
		await api.profile.install.mod(installId);
		await refresh();
	}

	async function install(id: ModId) {
		if (locked) return;
		installId = id;
		const prefs = await api.prefs.get();
		if (shouldWarnForeignDownload(id, prefs)) {
			foreignDownloadDialogOpen = true;
		} else {
			await doInstall();
		}
	}

	function onModClicked(evt: MouseEvent, mod: Mod, isInstalled: boolean) {
		if (evt.ctrlKey) {
			if (locked || (profiles.activeLocked && isInstalled)) return;
			installLatest(mod);
		} else if (selectedMod && itemUuid(selectedMod) === mod.uuid) {
			selectedMod = null;
			cachedSelectedKey = null;
		} else {
			selectedMod = findItemByUuid(mod.uuid);
			cachedSelectedKey = selectedMod ? (itemUuid(selectedMod) ?? null) : null;
		}
	}

	// A profile_changed event fires once per installed mod, so refreshing the
	// browse query on every one of them makes the window drag while a large
	// batch installs. Profile and query changes refresh immediately; mod-count
	// updates from an install are coalesced into one trailing query.
	let lastProfileId = untrack(() => profiles.activeId);
	let lastQuery = untrack(() => JSON.stringify(modQuery.current));
	let lastMaxCount = untrack(() => maxCount);
	let profileRefreshTimer: ReturnType<typeof setTimeout> | undefined;
	let firstRefresh = true;

	function scheduleBrowseRefresh() {
		if (profileRefreshTimer !== undefined) clearTimeout(profileRefreshTimer);
		profileRefreshTimer = setTimeout(() => {
			profileRefreshTimer = undefined;

			// don't re-query the catalog in the middle of a large batch; wait
			// until the install queue has drained, then refresh once
			if (installState.active) {
				scheduleBrowseRefresh();
				return;
			}

			untrack(() => refresh());
		}, 500);
	}

	$effect(() => {
		const profileId = profiles.activeId;
		// read all fields of modQuery.current to trigger the effect when any of them change
		const query = JSON.stringify(modQuery.current);
		const count = maxCount;
		// read the profile object so profile_changed updates schedule a refresh
		profiles.active;

		if (firstRefresh) {
			firstRefresh = false;
			lastProfileId = profileId;
			lastQuery = query;
			lastMaxCount = count;
			if (count > 0) untrack(() => refresh());
			return;
		}

		if (profileId !== lastProfileId || query !== lastQuery || count !== lastMaxCount) {
			lastProfileId = profileId;
			lastQuery = query;
			lastMaxCount = count;

			if (profileRefreshTimer !== undefined) {
				clearTimeout(profileRefreshTimer);
				profileRefreshTimer = undefined;
			}

			if (count > 0) untrack(() => refresh());
			return;
		}

		if (count <= 0) return;

		scheduleBrowseRefresh();
	});

	onDestroy(() => {
		if (profileRefreshTimer !== undefined) clearTimeout(profileRefreshTimer);
	});

	let locked = $derived(profiles.activeInstallAndConfigLocked);
</script>

<div class="flex grow overflow-hidden">
	<div class="flex w-[60%] grow flex-col overflow-hidden px-4 pt-4">
		<ModListFilters {sortOptions} bind:queryArgs={modQuery.current} />

		{#if locked}
			<ProfileLockedBanner class="mb-1" />
		{/if}

		{#key games.active?.slug}
			<ModList
				{mods}
				queryArgs={modQuery.current}
				bind:maxCount
				{scrollKey}
				rowId={(mod) =>
					itemUuid(mod) ??
					`${mod.data.thunderstore ? 'ts' : 'hx'}:${mod.data.thunderstore?.name ?? mod.data.hexium?.name ?? ''}`}
			>
				{#snippet placeholder()}
					{#if hasRefreshed}
						<HelpCard title={m.browse_modList_content_1()} icon="mdi:store-search" class="mt-4">
							{m.browse_modList_content_2()}
						</HelpCard>
					{/if}
				{/snippet}

				{#snippet item({ mod })}
					{@const shownMod = extractDeduplicatedMod(mod.data, getPreferredBackend(mod.data))!}
					{@const combinedDownloads =
						(mod.data.hexium?.downloads ?? 0) + (mod.data.thunderstore?.downloads ?? 0)}

					<ModListItem
						{contextItems}
						{combinedDownloads}
						mod={shownMod}
						isInstalled={mod.isInstalled}
						selected={selectedMod !== null && itemUuid(selectedMod) === shownMod.uuid}
						{locked}
						oninstall={() => installLatest(shownMod)}
						onclick={(evt) => onModClicked(evt, shownMod, mod.isInstalled)}
					/>
				{/snippet}
			</ModList>
		{/key}
	</div>

	{#if selectedMod}
		<DeduplicatedModDetails
			{locked}
			mod={selectedMod.data}
			{contextItems}
			onclose={() => {
				selectedMod = null;
				cachedSelectedKey = null;
			}}
		>
			{#snippet children({ mod })}
				<InstallModButton {mod} isInstalled={selectedMod?.isInstalled} {install} {locked} />
			{/snippet}
		</DeduplicatedModDetails>
	{/if}
</div>

<ForeignDownloadDialog bind:open={foreignDownloadDialogOpen} onConfirm={doInstall} />
