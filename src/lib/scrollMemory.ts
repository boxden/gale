import { page } from '$app/state';

/**
 * Scroll offsets remembered across client-side navigations, keyed by an
 * arbitrary string (usually the route path or an explicit list id).
 */
export const scrollPositions = new Map<string, number>();

/**
 * Scroll anchors for virtualized lists: which item index was at the top of
 * the viewport and the pixel offset into it. Unlike raw pixel offsets this
 * survives height-estimate drift — the restored position always lands on
 * the same item regardless of how accurate the list's height map is.
 */
export const scrollAnchors = new Map<string, { index: number; offset: number; id?: unknown }>();

// how long restore may keep retrying before giving up and accepting the
// live position — covers slow async content without looping forever
const RESTORE_TIMEOUT_MS = 4000;

/**
 * Svelte action that persists an element's scrollTop while the page is
 * alive and restores it when the page is mounted again. The restore is
 * retried while the content is still mutating, since async page content
 * can temporarily clamp the scroll position back to 0.
 *
 * Pass `null` to disable the action for an element.
 */
export function rememberScroll(node: HTMLElement, key: string | null = page.url.pathname) {
	if (key === null) return;

	const saved = scrollPositions.get(key);
	let restored = saved === undefined;
	let verifyRaf = 0;
	let timeout: ReturnType<typeof setTimeout> | undefined;

	const finish = () => {
		restored = true;
		if (timeout !== undefined) {
			clearTimeout(timeout);
			timeout = undefined;
		}
	};

	const tryRestore = () => {
		if (restored || saved === undefined) return;

		// wait until the content is tall enough to reach the saved offset
		if (node.scrollHeight - node.clientHeight < saved) return;

		node.scrollTop = saved;

		// verify on the next frame that the offset stuck; async content can
		// still be swapping in and clamping scrollTop back to 0
		verifyRaf = requestAnimationFrame(() => {
			verifyRaf = 0;
			if (restored) return;
			if (Math.abs(node.scrollTop - saved) <= 1) {
				finish();
			} else {
				tryRestore();
			}
		});
	};

	if (!restored) {
		// backstop: if the content never grows enough, stop retrying and
		// remember wherever the user actually ended up instead
		timeout = setTimeout(() => {
			finish();
			scrollPositions.set(key, node.scrollTop);
		}, RESTORE_TIMEOUT_MS);
	}

	const observer = new ResizeObserver(tryRestore);
	observer.observe(node);
	for (const child of node.children) observer.observe(child);
	// children added after mount (async lists) aren't covered by the
	// ResizeObserver above, so watch for DOM insertions too
	const mutations = new MutationObserver(tryRestore);
	mutations.observe(node, { childList: true, subtree: true });
	tryRestore();

	// write synchronously on every scroll event: rAF can be throttled or
	// delayed, losing the position if the page unmounts before the frame.
	// The scrollHeight guard skips the collapse-scroll fired while the DOM
	// is being torn down (it clamps scrollTop to 0). Writes are suspended
	// while a restore is pending so clamp-scrolls can't clobber `saved`.
	const onScroll = () => {
		if (restored && node.isConnected && node.scrollHeight > node.clientHeight) {
			scrollPositions.set(key, node.scrollTop);
		}
	};
	node.addEventListener('scroll', onScroll, { passive: true });

	return {
		destroy() {
			node.removeEventListener('scroll', onScroll);
			observer.disconnect();
			mutations.disconnect();
			if (verifyRaf !== 0) cancelAnimationFrame(verifyRaf);
			if (timeout !== undefined) clearTimeout(timeout);
			// no final write here: the DOM has already collapsed by the time
			// destroy runs, so scrollTop reads 0 and would clobber the saved
			// position. The scroll listener keeps the map fresh instead.
		}
	};
}
