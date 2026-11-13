<script module lang="ts">
	import type { Mod } from '$lib/types';

	// enough to keep both tabs' open mods and a few recently viewed ones, so
	// returning to a tab paints its readme in the first frame instead of
	// swapping the skeleton in afterwards
	const README_CACHE_MAX = 8;
	const readmeCache = new Map<string, string | null>();

	function readmeKeyOf(mod: Mod) {
		return `${mod.type}:${mod.backend}:${mod.uuid}:${mod.versionUuid}`;
	}

	function takeCachedReadme(key: string) {
		if (!readmeCache.has(key)) return undefined;

		const readme = readmeCache.get(key)!;
		readmeCache.delete(key);
		readmeCache.set(key, readme);
		return readme;
	}

	function storeReadme(key: string, readme: string | null) {
		readmeCache.delete(key);
		readmeCache.set(key, readme);

		if (readmeCache.size > README_CACHE_MAX) {
			const oldest = readmeCache.keys().next().value;
			if (oldest !== undefined) readmeCache.delete(oldest);
		}
	}
</script>

<script lang="ts">
	import Dialog from '$lib/components/ui/Dialog.svelte';
	import Markdown from '$lib/components/ui/Markdown.svelte';

	import ModInfoDialog from '../dialogs/ModInfoDialog.svelte';
	import ModCardList from '../ui/ModCardList.svelte';
	import { rememberScroll } from '$lib/scrollMemory';

	import { type ContextItem, ModType } from '$lib/types';
	import {
		communityUrl,
		formatModName,
		getMarkdown,
		modIconSrc,
		shortenFileSize,
		shortenNum,
		timeSince
	} from '$lib/util';

	import { DropdownMenu } from 'bits-ui';

	import Icon from '@iconify/svelte';
	import { untrack, type Snippet } from 'svelte';
	import { page } from '$app/state';
	import { m } from '$lib/paraglide/messages';
	import config from '$lib/state/config.svelte';
	import InfoBox from '../ui/InfoBox.svelte';
	import Tooltip from '../ui/Tooltip.svelte';
	import games from '$lib/state/game.svelte';
	import ContextMenuContent from '../ui/ContextMenuContent.svelte';

	type Props = {
		mod: Mod;
		contextItems?: ContextItem[];
		onclose: () => void;
		header?: Snippet;
		children?: Snippet;
		hideBackend?: boolean;
		configFile?: string | null;
	};

	let {
		mod,
		contextItems = [],
		onclose,
		header,
		children,
		hideBackend: hideBackendProp,
		configFile
	}: Props = $props();

	let dependenciesOpen = $state(false);
	let suggestionsOpen = $state(false);

	let readmeOpen = $state(false);
	let readme: string | null | undefined = $state(
		untrack(() => takeCachedReadme(readmeKeyOf(mod)))
	);

	let changelogOpen = $state(false);
	let changelogPromise: Promise<string | null> | null = $state(null);

	let allContextItems = $derived([
		...contextItems,
		{
			label: m.modDetails_allContextItems_close(),
			icon: 'mdi:close',
			onclick: onclose
		}
	]);

	const showBackend = $derived(!hideBackendProp && games.activeBackends.length > 1);

	function formatReadme(readme: string) {
		return readme
			.split('\n')
			.filter((line) => !line.startsWith('# '))
			.join('\n');
	}

	function openChangelog() {
		if (!changelogPromise) {
			fetchChangelog();
		}
		changelogOpen = true;
	}

	function fetchChangelog() {
		changelogPromise = getMarkdown(mod, 'changelog', true);
	}

	// key on readme identity so a list refresh re-resolving the mod doesn't refetch and collapse the restored scroll
	const readmeKey = $derived(readmeKeyOf(mod));

	// the panel is the same component for every mod, so the scroll key must
	// include the mod (and the tab it was opened from) to never restore one
	// mod's readme position onto another
	const scrollKey = $derived(`mod-details:${page.url.pathname}:${readmeKey}`);

	$effect(() => {
		const key = readmeKey;
		readme = takeCachedReadme(key);
		getMarkdown(untrack(() => mod), 'readme').then((result) => {
			storeReadme(key, result);
			// a slower response for a mod that was swapped out must not
			// overwrite the readme currently on screen
			if (readmeKeyOf(mod) === key) readme = result;
		});
	});
</script>

<div class="text-primary-900 relative flex w-[40%] min-w-72 flex-col p-4 pl-2 dark:text-white">
	<DropdownMenu.Root>
		<DropdownMenu.Trigger
			class="dark:hover:bg-primary-700 hover:bg-primary-100 absolute right-4 mt-1 rounded-full p-1"
		>
			<Icon class="text-primary-700 dark:text-primary-200 text-3xl" icon="mdi:dots-vertical" />
		</DropdownMenu.Trigger>
		<ContextMenuContent items={allContextItems} type="dropdown" />
	</DropdownMenu.Root>

	<div
		class="-mr-3 grow overflow-x-hidden overflow-y-scroll pr-3 pb-2"
		use:rememberScroll={scrollKey}
	>
		<div class="mb-3 flex flex-col gap-4 xl:flex-row xl:items-center">
			<img src={modIconSrc(mod)} class="max-h-30 max-w-30 rounded-lg" alt="" />

			<div>
				<svelte:element
					this={mod.type === ModType.Remote ? 'a' : 'div'}
					class={[
						'text-primary-900 block pr-4 text-left text-2xl font-semibold xl:text-4xl xl:font-bold dark:text-white',
						mod.type === ModType.Remote && 'hover:underline'
					]}
					href={communityUrl(mod.backend, mod.author ?? '', mod.name)}
					target="_blank">{formatModName(mod.name)}</svelte:element
				>

				{#if mod.author}
					<a
						class="text-primary-500 hover:text-primary-600 dark:text-primary-400 dark:hover:text-primary-300 block text-lg hover:underline xl:text-xl"
						href={communityUrl(mod.backend, mod.author)}
						target="_blank"
					>
						{mod.author}
					</a>
				{/if}

				{#if mod.version}
					<div class="text-primary-500 dark:text-primary-400 text-lg xl:text-xl">{mod.version}</div>
				{/if}
			</div>
		</div>

		{@render header?.()}

		{#if mod.isDeprecated}
			<InfoBox type="warning">
				{m.modDetails_deprecated()}
			</InfoBox>
		{/if}

		{#if mod.containsNsfw}
			<InfoBox type="warning">
				{m.modDetails_NSFW()}
			</InfoBox>
		{/if}

		{#if mod.categories && mod.categories.length > 0}
			<div class="mt-2 mb-1 flex flex-wrap gap-1">
				{#each mod.categories as category}
					<div
						class="dark:bg-primary-700 dark:text-primary-200 text-primary-700 bg-primary-200 rounded-full px-3 py-1 text-sm"
					>
						{category}
					</div>
				{/each}
			</div>
		{/if}

		<div class="mt-3 flex items-center gap-1.5 text-lg">
			{#if mod.rating !== null}
				<Icon class="shrink-0 text-yellow-500" icon="mdi:star" />
				<span class="mr-4 text-yellow-500">{shortenNum(mod.rating)}</span>
			{/if}
			{#if mod.downloads !== null}
				<Icon class="shrink-0 text-green-600 dark:text-green-500" icon="mdi:download" />
				<span class="mr-4 text-green-600 dark:text-green-500">{shortenNum(mod.downloads)}</span>
			{/if}
			<Icon class="text-primary-500 dark:text-primary-400 shrink-0" icon="mdi:weight" />
			<span class="text-primary-500 dark:text-primary-400">{shortenFileSize(mod.fileSize)}</span>
		</div>

		{#if showBackend || mod.lastUpdated}
			<div
				class="text-primary-500 dark:text-primary-400 mt-1.5 mb-3 flex flex-wrap items-center gap-x-2"
			>
				{#if showBackend}
					<span>{mod.backend}</span>
				{/if}

				{#if showBackend && mod.lastUpdated}
					<span>•</span>
				{/if}

				{#if mod.lastUpdated}
					<Tooltip
						class="border-primary-400 border-b border-dashed"
						text={new Date(mod.lastUpdated).toLocaleString()}
					>
						{m.modDetails_lastUpdated({ time: timeSince(new Date(mod.lastUpdated)) })}
					</Tooltip>
				{/if}
			</div>
		{/if}

		{#if mod.description !== null}
			<p class="text-primary-600 dark:text-primary-300 text-xl lg:hidden">
				{mod.description}
			</p>
		{/if}

		<div class="hidden lg:block">
			{#if readme === undefined}
				<div role="status" class="animate-pulse">
					<div class="dark:bg-primary-700 bg-primary-200 mt-4 h-8 w-80 rounded-xl"></div>
					<div class="dark:bg-primary-700 bg-primary-200 mt-6 h-3 max-w-125 rounded-full"></div>
					<div class="dark:bg-primary-700 bg-primary-200 mt-2.5 h-3 max-w-115 rounded-full"></div>
					<div
						class="dark:bg-primary-700 bg-primary-200 mt-2.5 mb-4 h-3 max-w-100 rounded-full"
					></div>
				</div>
			{:else}
				<Markdown source={readme ? formatReadme(readme) : m.modDetails_noFound()} />
			{/if}
		</div>
	</div>

	{#if configFile}
		<div
			class="text-accent-600 hover:text-accent-700 dark:text-accent-400 dark:hover:text-accent-300 my-2 flex items-center gap-2 text-lg hover:underline"
		>
			<Icon class="text-xl" icon="mdi:file-cog" />
			<button onclick={() => config.gotoModConfig(configFile)}>
				{m.modDetails_editConfig()}
			</button>
		</div>
	{/if}

	{#snippet button(icon: string, label: string, onclick: () => void, onmouseenter?: () => void)}
		<button
			class="group dark:bg-primary-700 dark:hover:bg-primary-600 dark:text-primary-200 text-primary-700 bg-primary-200 hover:bg-primary-300 mb-1 flex items-center rounded-md py-1 pr-1.5 pl-3"
			{onmouseenter}
			{onclick}
		>
			<Icon {icon} class="mr-2 text-lg" />
			{label}
		</button>
	{/snippet}

	{@render button('mdi:file-document', m.modDetails_changeLog(), openChangelog, fetchChangelog)}
	{@render button('mdi:info', m.modDetails_details(), () => (readmeOpen = true))}

	{#if mod.dependencies !== null && mod.dependencies.length > 0}
		{@render button(
			'mdi:dependency',
			`${m.modDetails_dependencies()} (${mod.dependencies.length})`,
			() => (dependenciesOpen = true)
		)}
	{/if}

	{#if mod.suggestions !== null && mod.suggestions.length > 0}
		{@render button(
			'mdi:lightbulb',
			`${m.modDetails_suggestions()} (${mod.suggestions.length})`,
			() => (suggestionsOpen = true)
		)}
	{/if}

	{@render children?.()}
</div>

<Dialog title={m.modDetails_dependencies_title({ name: mod.name })} bind:open={dependenciesOpen}>
	{#if mod.dependencies}
		<ModCardList
			mods={mod.dependencies.map((fullName) => ({ fullName, backend: mod.backend }))}
			class="mt-4"
		/>
	{/if}
</Dialog>

<Dialog title={m.modDetails_suggestions_title({ name: mod.name })} bind:open={suggestionsOpen}>
	{#if mod.suggestions}
		<ModCardList
			mods={mod.suggestions.map((fullName) => ({ fullName, backend: mod.backend }))}
			class="mt-4"
		/>
	{/if}
</Dialog>

{#if readme !== undefined}
	<ModInfoDialog bind:open={readmeOpen} content={readme} type="readme" />
{/if}

{#await changelogPromise then changelog}
	<ModInfoDialog bind:open={changelogOpen} content={changelog} type="changelog" />
{/await}
