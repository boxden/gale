<script lang="ts" generics="T, Key = number">
	import { run } from 'svelte/legacy';

	// (c) 2018 Rich Harris
	// https://github.com/sveltejs/svelte-virtual-list/blob/master/LICENSE
	import { onDestroy, onMount, tick, type Snippet } from 'svelte';

	type Props = {
		// props
		items: T[];
		height?: string;
		itemHeight?: number | undefined;
		// read-only, but visible to consumers via bind:start
		start?: number;
		end?: number;
		rowId?: (data: T) => Key;
		children?: Snippet<[{ item: T; index: number }]>;
	};

	let {
		items,
		height = '100%',
		itemHeight = undefined,
		start = $bindable(0),
		end = $bindable(0),
		rowId,
		children
	}: Props = $props();

	// local state
	const OVERSCAN = 3;

	let heightMap: number[] = [];
	let rows: HTMLCollectionOf<HTMLElement>;
	let viewport: HTMLElement;
	let contents: HTMLElement;
	let viewportHeight = $state(0);
	let renderStart = $derived(Math.max(0, start - OVERSCAN));
	let renderEnd = $derived(Math.min(items.length, end + OVERSCAN));
	let visible: { index: number; data: T }[] = $derived(
		items.slice(renderStart, renderEnd).map((data, i) => {
			return { index: i + renderStart, data };
		})
	);
	let mounted: boolean = $state(false);

	let top = $state(0);
	let bottom = $state(0);
	let average_height: number;

	let padTop = $derived.by(() => {
		let pad = top;
		for (let i = renderStart; i < start; i++) pad -= heightMap[i] || average_height || 0;
		return Math.max(0, pad);
	});

	let padBottom = $derived.by(() => {
		let pad = bottom;
		for (let i = end; i < renderEnd; i++) pad -= heightMap[i] || average_height || 0;
		return Math.max(0, pad);
	});

	async function refresh(items: T[], viewportHeight: number, itemHeight: number | undefined) {
		const { scrollTop } = viewport;

		await tick(); // wait until the DOM is up to date

		let contentHeight = top - scrollTop;
		let i = start;
		const rs = Math.max(0, start - OVERSCAN);

		while (contentHeight < viewportHeight && i < items.length) {
			let row = rows[i - rs];

			if (!row) {
				end = i + 1;
				await tick(); // render the newly visible row
				row = rows[i - rs];
			}

			const rowHeight = (heightMap[i] = itemHeight ?? row.offsetHeight);
			contentHeight += rowHeight;
			i += 1;
		}

		end = i;

		const remaining = items.length - end;
		average_height = end > 0 ? (top + contentHeight) / end : 0;

		bottom = remaining * average_height;
		heightMap.length = items.length;
	}

	let scrollRaf = 0;

	function onScrollRequest() {
		if (scrollRaf !== 0) return;
		scrollRaf = requestAnimationFrame(() => {
			scrollRaf = 0;
			handleScroll();
		});
	}

	async function handleScroll() {
		const { scrollTop } = viewport;

		const old_start = start;
		const rs = Math.max(0, start - OVERSCAN);

		for (let v = 0; v < rows.length; v += 1) {
			heightMap[rs + v] = itemHeight || rows[v].offsetHeight;
		}

		let i = 0;
		let y = 0;

		while (i < items.length) {
			const row_height = heightMap[i] || average_height;
			if (y + row_height > scrollTop) {
				start = i;
				top = y;

				break;
			}

			y += row_height;
			i += 1;
		}

		while (i < items.length) {
			y += heightMap[i] || average_height;
			i += 1;

			if (y > scrollTop + viewportHeight) break;
		}

		end = i;

		const remaining = items.length - end;
		average_height = end > 0 ? y / end : 0;

		while (i < items.length) heightMap[i++] = average_height;
		bottom = remaining * average_height;

		// prevent jumping if we scrolled up into unknown territory
		if (start < old_start) {
			await tick();

			const rsOld = Math.max(0, old_start - OVERSCAN);
			let expected_height = 0;
			let actual_height = 0;

			for (let i = start; i < old_start; i += 1) {
				if (rows[i - rsOld]) {
					expected_height += heightMap[i];
					actual_height += itemHeight || rows[i - rsOld].offsetHeight;
				}
			}

			const d = actual_height - expected_height;
			if (d !== 0) {
				viewport.scrollTo(0, scrollTop + d);
			}
		}

		// TODO if we overestimated the space these
		// rows would occupy we may need to add some
		// more. maybe we can just call handle_scroll again?
	}

	export function scrollTo(y: number) {
		viewport.scrollTo(0, y);
	}

	// trigger initial refresh
	onMount(() => {
		rows = contents.getElementsByTagName(
			'svelte-virtual-list-row'
		) as HTMLCollectionOf<HTMLElement>;
		mounted = true;
	});

	onDestroy(() => {
		if (scrollRaf !== 0) cancelAnimationFrame(scrollRaf);
	});

	// whenever `items` changes, invalidate the current heightmap
	run(() => {
		if (mounted) refresh(items, viewportHeight, itemHeight);
	});
</script>

<svelte-virtual-list-viewport
	bind:this={viewport}
	bind:offsetHeight={viewportHeight}
	onscroll={onScrollRequest}
	style="height: {height};"
>
	<svelte-virtual-list-contents
		bind:this={contents}
		style="padding-top: {padTop}px; padding-bottom: {padBottom}px;"
	>
		{#each visible as row (rowId?.(row.data) ?? row.index)}
			<svelte-virtual-list-row>
				{#if children}{@render children({ item: row.data, index: row.index })}{:else}Missing
					template{/if}
			</svelte-virtual-list-row>
		{/each}
	</svelte-virtual-list-contents>
</svelte-virtual-list-viewport>

<style>
	svelte-virtual-list-viewport {
		position: relative;
		overflow-y: scroll;
		-webkit-overflow-scrolling: touch;
		display: block;
	}

	svelte-virtual-list-contents,
	svelte-virtual-list-row {
		display: block;
	}
</style>
