<script lang="ts" generics="T">
	import VirtualList from '$lib/components/ui/VirtualList.svelte';
	import { storeLoaded } from '$lib/state/persisted-state.svelte';
	import type { Mod, QueryModsArgsWithoutMax } from '$lib/types';
	import type { Snippet } from 'svelte';

	type Props = {
		mods: T[];
		maxCount: number;
		queryArgs: QueryModsArgsWithoutMax;
		placeholder?: Snippet;
		item: Snippet<[{ mod: T; index: number }]>;
		rowId?: (mod: T) => string;
		scrollKey?: string;
	};

	let {
		mods,
		maxCount = $bindable(20),
		queryArgs,
		placeholder,
		item,
		rowId,
		scrollKey
	}: Props = $props();

	let listStart = $state(0);
	let listEnd = $state(0);
	let virtualList: VirtualList<T, string> | null = $state(null);

	$effect(() => {
		if (listEnd > mods.length - 4 && mods.length === maxCount) {
			maxCount += 20;
		}
	});

	// queryArgs swaps from defaults to the persisted value once the store
	// hydrates; that isn't a user-initiated query change and must not reset
	// a restored scroll position
	let storeHydrated = $state(false);
	void storeLoaded.then(() => (storeHydrated = true));

	let lastQueryArgs: string | undefined;
	$effect(() => {
		const q = JSON.stringify(queryArgs);
		if (storeHydrated && lastQueryArgs !== undefined && lastQueryArgs !== q) {
			virtualList?.scrollTo(0);
		}
		lastQueryArgs = q;
	});
</script>

{#if mods.length === 0}
	<div class="text-primary-600 dark:text-primary-300 text-center">
		{@render placeholder?.()}
	</div>
{:else}
	<VirtualList
		{rowId}
		{scrollKey}
		items={mods}
		bind:this={virtualList}
		bind:start={listStart}
		bind:end={listEnd}
	>
		{#snippet children({ item: mod, index })}
			{@render item({
				mod,
				index
			})}
		{/snippet}
	</VirtualList>
{/if}
