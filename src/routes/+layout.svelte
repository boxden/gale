<script lang="ts">
	import '../app.css';

	import { Tooltip } from 'bits-ui';

	import Menubar from '$lib/components/menubar/Menubar.svelte';
	import Toolbar from '$lib/components/toolbar/Toolbar.svelte';
	import FetchModsBar from '$lib/components/misc/FetchModsBar.svelte';
	import Toasts from '$lib/components/misc/Toasts.svelte';

	import { onMount, type Snippet } from 'svelte';
	import InstallModDialog from '$lib/components/dialogs/InstallModDialog.svelte';
	import WelcomeDialog from '$lib/components/dialogs/WelcomeDialog.svelte';
	import Navbar from '$lib/components/misc/Navbar.svelte';
	import profiles from '$lib/state/profile.svelte';
	import { updateBanner } from '$lib/state/misc.svelte';
	import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { invoke } from '$lib/invoke';
	import type { ProfileInfo, ManagedGameInfo } from '$lib/types';
	import { refreshLanguage } from '$lib/i18n';
	import { storeLoaded } from '$lib/state/persisted-state.svelte';
	import MissingProfilesDialog from '$lib/components/dialogs/MissingProfilesDialog.svelte';

	type Props = {
		children?: Snippet;
	};

	let { children }: Props = $props();

	let unlistenProfiles: UnlistenFn | null;
	let unlistenGames: UnlistenFn | null;

	onMount(() => {
		refreshLanguage();

		$effect(() => {
			profiles.activeId;
			updateBanner.threshold = 0;
		});

		listen<ProfileInfo>('profile_changed', (evt) => {
			profiles.updateOne(evt.payload);
		}).then((callback) => (unlistenProfiles = callback));

		listen<ManagedGameInfo>('game_changed', (evt) => {
			profiles.update(evt.payload);
		}).then((callback) => (unlistenGames = callback));

		void storeLoaded.then(async () => {
			// let pending theme effects (dark class, color shades) flush
			await new Promise((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(resolve))
			);
			document.documentElement.classList.remove('theme-loading');

			// restore the saved window geometry once per session; re-applying
			// it on page reloads (e.g. language switch) would reset the window
			// while it's visible
			if (!sessionStorage.getItem('window-state-restored')) {
				invoke('plugin:window-state|restore_state', { label: 'main' })
					.then(() => sessionStorage.setItem('window-state-restored', 'true'))
					.catch(() => {});
			}

			// match the webview's default background to the app theme so the
			// first visible frame doesn't flash a foreign color
			const bg = getComputedStyle(document.querySelector('main')!).backgroundColor;
			const rgb = bg.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
			if (rgb) {
				invoke('set_window_background_color', {
					red: Number(rgb[1]),
					green: Number(rgb[2]),
					blue: Number(rgb[3])
				}).catch(() => {});
			}

			// wait for the first painted frames so the window isn't shown mid-render
			setTimeout(() => emit('ready'), 100);
		});

		return () => {
			unlistenProfiles?.();
			unlistenGames?.();
		};
	});
</script>

<svelte:body
	oncontextmenu={(evt) => {
		// hide context menu in release builds
		if (window.location.hostname === 'tauri.localhost') {
			evt.preventDefault();
		}
	}}
/>

<Tooltip.Provider skipDelayDuration={1} disableCloseOnTriggerClick>
	<main class="dark:bg-primary-800 relative flex flex-col bg-white">
		<Menubar />
		<Toolbar />

		<div class="relative flex grow overflow-hidden">
			<Navbar />

			{@render children?.()}
		</div>

		<FetchModsBar />
		<Toasts />
	</main>

	<InstallModDialog />
	<WelcomeDialog />
	<MissingProfilesDialog />
</Tooltip.Provider>
