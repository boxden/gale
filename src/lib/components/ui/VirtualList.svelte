<script lang="ts" generics="T, Key = number">
	import { run } from 'svelte/legacy';

	// (c) 2018 Rich Harris
	// https://github.com/sveltejs/svelte-virtual-list/blob/master/LICENSE
	import { onDestroy, onMount, tick, untrack, type Snippet } from 'svelte';
	import { scrollAnchors } from '$lib/scrollMemory';

	type Props = {
		// props
		items: T[];
		height?: string;
		itemHeight?: number | undefined;
		// read-only, but visible to consumers via bind:start
		start?: number;
		end?: number;
		rowId?: (data: T) => Key;
		scrollKey?: string;
		children?: Snippet<[{ item: T; index: number }]>;
	};

	let {
		items,
		height = '100%',
		itemHeight = undefined,
		start = $bindable(0),
		end = $bindable(0),
		rowId,
		scrollKey,
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

		// row pitch includes margins that offsetHeight leaves out, so measure
		// the rendered window before deriving averages from it
		measureRenderedRows(rs);

		let measured = 0;
		for (let k = start; k < end; k += 1) measured += heightMap[k] || 0;

		const remaining = items.length - end;
		average_height = end > 0 ? (top + measured) / end : 0;

		bottom = remaining * average_height;
		heightMap.length = items.length;

		// let the spacer padding flush to the DOM so scrollTo isn't clamped
		// to the still-small scrollHeight
		await tick();

		await tryScrollRestore();
	}

	// offsetHeight excludes margins that collapse out of a row, while the
	// distance between adjacent rows includes them. The virtual positions must
	// follow the visible pitch, otherwise every re-measure shifts the content
	// by the accumulated margin error.
	function measureRenderedRows(rs: number) {
		if (itemHeight !== undefined) {
			for (let v = 0; v < rows.length; v += 1) heightMap[rs + v] = itemHeight;
			return;
		}

		let gap = 0;
		for (let v = 0; v + 1 < rows.length; v += 1) {
			const pitch = rows[v + 1].offsetTop - rows[v].offsetTop;
			heightMap[rs + v] = pitch;
			gap = pitch - rows[v].offsetHeight;
		}

		if (rows.length > 0) {
			const last = rows.length - 1;
			heightMap[rs + last] = rows[last].offsetHeight + Math.max(0, gap);
		}
	}

	// scroll restore: the saved anchor is snapshotted once so clamp-scrolls
	// fired while async content streams in can't degrade it. The viewport is
	// hidden until the restore settles so the list never paints at the top
	// and then visibly jumps.
	let scrollRestoreDone = $state(false);
	type ScrollAnchor = { index: number; offset: number; id?: unknown };
	let scrollAnchor: ScrollAnchor | null | undefined = $state(
		untrack(() => (scrollKey === undefined ? undefined : (scrollAnchors.get(scrollKey) ?? null)))
	);
	let restorePending = $derived(
		scrollAnchor !== null && scrollAnchor !== undefined && !scrollRestoreDone
	);
	let restoreTimer: ReturnType<typeof setTimeout> | undefined;
	let settleTimer: ReturnType<typeof setTimeout> | undefined;
	// set when the restore scrollTo fires; the scroll event it triggers runs
	// one more handleScroll pass whose compensation may adjust the position,
	// so the reveal waits for that pass
	let restoreScrollFired = false;

	function finishScrollRestore() {
		if (scrollRestoreDone) return;
		scrollRestoreDone = true;
		if (restoreTimer !== undefined) {
			clearTimeout(restoreTimer);
			restoreTimer = undefined;
		}
		if (settleTimer !== undefined) {
			clearTimeout(settleTimer);
			settleTimer = undefined;
		}
	}

	// item at the current scrollTop plus the pixel offset into it; uses the
	// rendered rows, so it is exact and independent of the height estimates.
	// Returns null while scrollTop is outside the rendered window — the
	// handleScroll pass that follows will re-render and resolve it.
	function currentAnchor(): ScrollAnchor | null {
		const y = viewport.scrollTop;
		const rs = Math.max(0, start - OVERSCAN);

		for (let v = 0; v < rows.length; v += 1) {
			const row = rows[v];
			if (row.offsetTop + row.offsetHeight > y) {
				const index = rs + v;
				const item = items[index];
				return {
					index,
					offset: y - row.offsetTop,
					id: item !== undefined && rowId ? rowId(item) : undefined
				};
			}
		}

		return null;
	}

	function saveAnchor() {
		if (
			scrollKey === undefined ||
			!scrollRestoreDone ||
			!viewport.isConnected ||
			viewport.scrollHeight <= viewport.clientHeight
		) {
			return;
		}

		const anchor = currentAnchor();
		if (anchor !== null) scrollAnchors.set(scrollKey, anchor);
	}

	async function tryScrollRestore() {
		if (scrollRestoreDone || scrollKey === undefined) return;

		// the anchor was read eagerly at mount; none means this is the first
		// visit and there is nothing to restore
		if (scrollAnchor === null) {
			finishScrollRestore();
			return;
		}

		if (scrollAnchor === undefined || items.length === 0) return;

		// never leave the hidden viewport hidden indefinitely
		restoreTimer ??= setTimeout(finishScrollRestore, 1000);

		const anchor = { ...scrollAnchor };

		// resolve the row identity if it is already loaded; the list may have
		// been reordered since the anchor was saved. While results are still
		// streaming in the row may simply be absent, so fall back to the
		// stored index rather than waiting for it.
		if (anchor.id !== undefined && rowId) {
			const index = items.findIndex((item) => rowId(item) === anchor.id);
			if (index >= 0) anchor.index = index;
		}

		// the catalog may have shrunk since the position was saved
		if (anchor.index >= items.length) {
			anchor.index = items.length - 1;
			anchor.offset = 0;
		}

		// render a window around the anchor row so its position is measurable
		const rs = Math.max(0, anchor.index - OVERSCAN);
		if (start !== rs || end < anchor.index + 1) {
			start = rs;
			end = Math.min(items.length, anchor.index + OVERSCAN + 1);
		}

		// keep the spacer estimates consistent with the window we just
		// positioned, otherwise padTop collapses and row.offsetTop is
		// measured against the wrong origin
		let y = 0;
		for (let i = 0; i < start; i += 1) y += heightMap[i] || average_height || 0;
		top = y;
		bottom = (items.length - end) * (average_height || 0);
		await tick();

		const rsNow = Math.max(0, start - OVERSCAN);
		const row = rows[anchor.index - rsNow];
		if (row === undefined) return;

		// the saved offset may point past the end of the row if its content
		// changed while the list was unmounted
		const anchorTop = row.offsetTop;
		const offset = Math.max(0, Math.min(anchor.offset, (itemHeight ?? row.offsetHeight) - 1));
		const target = anchorTop + offset;
		if (viewport.scrollHeight - viewport.clientHeight < target) return;

		// measure the rendered window before re-positioning it, so the
		// normalization below doesn't fall back to height estimates
		measureRenderedRows(rsNow);

		// margins can collapse out of the rendered window, making the first
		// row sit a few pixels below padTop; that offset is not part of the
		// virtual position of the rows
		const lead = rows.length > 0 ? rows[0].offsetTop - padTop : 0;

		// make the anchor row the window's first row: the scroll event fired
		// by the restore then compensates against the row that is actually
		// visible instead of an offscreen overscan row, so estimate errors
		// can't shift the remembered offset a little on every visit
		start = anchor.index;
		top = anchorTop - lead;

		scrollAnchors.set(scrollKey, { ...anchor, offset });
		viewport.scrollTo(0, target);
		restoreScrollFired = true;
		// if the scrollTo doesn't produce a scroll event, don't stay hidden
		settleTimer = setTimeout(() => {
			settleTimer = undefined;
			if (restoreScrollFired) {
				restoreScrollFired = false;
				finishScrollRestore();
			}
		}, 300);
	}

	let scrollRaf = 0;

	function onScrollRequest() {
		// write synchronously: rAF can be throttled or delayed, and a position
		// saved only on the next frame may be lost if the page unmounts first
		saveAnchor();
		if (scrollRaf !== 0) return;
		scrollRaf = requestAnimationFrame(() => {
			scrollRaf = 0;
			void handleScroll();
		});
	}

	let handlingScroll = false;
	let scrollPending = false;

	async function handleScroll() {
		// handleScroll suspends on tick() while the DOM re-renders; a scroll
		// event arriving in that window must not start a concurrent pass
		if (handlingScroll) {
			scrollPending = true;
			return;
		}
		handlingScroll = true;
		try {
			do {
				scrollPending = false;
				await handleScrollUpdate();
			} while (scrollPending);
		} finally {
			handlingScroll = false;
		}
	}

	async function handleScrollUpdate() {
		const { scrollTop } = viewport;

		const old_start = start;
		const rs = Math.max(0, start - OVERSCAN);

		// visual anchor: document-space Y of the row at the viewport top,
		// captured while the old window is still in the DOM. After the
		// window slides and heights are re-measured, any shift of this
		// row's document position is compensated in scrollTop — this keeps
		// the content visually stationary regardless of how wrong the
		// height estimates were.
		const anchorIndex = old_start;
		const anchorRow = rows[anchorIndex - rs];
		const anchorTop = anchorRow === undefined ? null : anchorRow.offsetTop;

		measureRenderedRows(rs);

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

		// fill unmeasured slots with the height of the nearest measured row
		// above — row heights are locally similar, so propagating the last
		// real measurement is much more accurate than the global average.
		// Inaccurate estimates here inflate/deflate padTop and make the
		// scroll anchoring compensation pin the content in place.
		let lastHeight = average_height || 0;
		for (let j = 0; j < items.length; j += 1) {
			const h = heightMap[j];
			if (h) {
				lastHeight = h;
			} else {
				heightMap[j] = lastHeight;
			}
		}
		bottom = remaining * average_height;

		// compensate the visual shift of the anchored row. The previous
		// implementation summed height deltas over an index range, but
		// overscan rows sit inside both sides of the padTop subtraction
		// and never move the content — the resulting scrollTo calls just
		// dragged scrollTop away from the user's position (visible as
		// pinned/jumpy scrolling through unmeasured regions).
		if (anchorTop !== null) {
			await tick();

			const rsNow = Math.max(0, start - OVERSCAN);
			const row = rows[anchorIndex - rsNow];
			if (row !== undefined) {
				const d = row.offsetTop - anchorTop;
				if (Math.abs(d) > 0.5) {
					viewport.scrollTo(0, viewport.scrollTop + d);
				}
			}
		}

		// the post-restore scroll pass has run its compensation — reveal
		// the list at its final position
		if (restoreScrollFired) {
			restoreScrollFired = false;
			finishScrollRestore();
		}

		saveAnchor();
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
		if (restoreTimer !== undefined) clearTimeout(restoreTimer);
		if (settleTimer !== undefined) clearTimeout(settleTimer);
		// no final write here: the DOM has already collapsed by the time
		// onDestroy runs, so scrollTop reads 0 and would clobber the saved
		// position. saveAnchor keeps the map fresh while scrolling instead.
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
	style="height: {height}; visibility: {restorePending ? 'hidden' : 'visible'};"
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
		/* compensation is done explicitly; browser anchoring would adjust
		   scrollTop a second time when the rendered window shifts */
		overflow-anchor: none;
	}

	svelte-virtual-list-contents,
	svelte-virtual-list-row {
		display: block;
	}
</style>
