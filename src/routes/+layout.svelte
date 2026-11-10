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
	import { darkMode, primaryColorSetting } from '$lib/state/theme.svelte';
	import MissingProfilesDialog from '$lib/components/dialogs/MissingProfilesDialog.svelte';

	type Props = {
		children?: Snippet;
	};

	let { children }: Props = $props();

	function syncWindowBackground() {
		const main = document.querySelector('main');
		if (!main) return;

		// computed theme colors can be oklch, which the canvas normalizes to
		// the sRGB bytes the window background API expects
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 1;
		const ctx = canvas.getContext('2d', { willReadFrequently: true });
		if (!ctx) return;

		ctx.fillStyle = getComputedStyle(main).backgroundColor;
		ctx.fillRect(0, 0, 1, 1);
		const [red, green, blue] = ctx.getImageData(0, 0, 1, 1).data;

		invoke('set_window_background_color', { red, green, blue }).catch(() => {});
	}

	// the webview paints its own background in the gaps exposed while the
	// window is resized, so it must follow runtime theme changes too
	$effect(() => {
		darkMode.current;
		primaryColorSetting.current;

		let inner = 0;
		const outer = requestAnimationFrame(() => {
			inner = requestAnimationFrame(syncWindowBackground);
		});

		return () => {
			cancelAnimationFrame(outer);
			cancelAnimationFrame(inner);
		};
	});

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

			// match the webview's default background to the app theme so the
			// first visible frame doesn't flash a foreign color
			syncWindowBackground();

			// reveal after the next painted frame instead of an arbitrary delay
			requestAnimationFrame(() => requestAnimationFrame(() => emit('ready')));
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
