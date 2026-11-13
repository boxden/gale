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

// a pending restore must never fight the user: a wheel or touch scroll is
// unambiguous intent, so accept whatever position they pick from then on
const USER_SCROLL_EVENTS = ['wheel', 'touchstart'] as const;

type ScrollMemoryHandle = {
	flush: () => void;
	stop: () => void;
};

function trackScroll(node: HTMLElement, key: string, restore: boolean): ScrollMemoryHandle {
	const saved = scrollPositions.get(key);
	let restored = !restore || saved === undefined;
	let verifyRaf = 0;
	let timeout: ReturnType<typeof setTimeout> | undefined;
	let observer: ResizeObserver | undefined;
	let mutations: MutationObserver | undefined;

	const finish = () => {
		restored = true;
		if (timeout !== undefined) {
			clearTimeout(timeout);
			timeout = undefined;
		}
		// once the position is restored, the observers only add work to every
		// window resize; the scroll listener keeps the saved offset fresh
		observer?.disconnect();
		mutations?.disconnect();
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
		// backstop: if the content never grows enough, stop retrying. Keep the
		// remembered offset unless the content actually moved, so a shorter
		// render this time doesn't erase a position that is still valid
		timeout = setTimeout(() => {
			finish();
			if (node.scrollTop > 0) scrollPositions.set(key, node.scrollTop);
		}, RESTORE_TIMEOUT_MS);
	}

	observer = new ResizeObserver(tryRestore);
	observer.observe(node);
	for (const child of node.children) observer.observe(child);
	// children added after mount (async lists) aren't covered by the
	// ResizeObserver above, so watch for DOM insertions too
	mutations = new MutationObserver(tryRestore);
	mutations.observe(node, { childList: true, subtree: true });
	tryRestore();

	// first visit: there is nothing to restore, so don't keep observing
	if (restored) {
		observer.disconnect();
		mutations.disconnect();
	}

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

	// the scroll event that follows a wheel/touch gesture records the new
	// position; this only has to stop the restore from pulling it back
	const onUserScroll = () => {
		if (!restored) finish();
	};
	for (const event of USER_SCROLL_EVENTS) {
		node.addEventListener(event, onUserScroll, { passive: true });
	}

	return {
		flush: onScroll,
		stop() {
			node.removeEventListener('scroll', onScroll);
			for (const event of USER_SCROLL_EVENTS) {
				node.removeEventListener(event, onUserScroll);
			}
			observer?.disconnect();
			mutations?.disconnect();
			if (verifyRaf !== 0) cancelAnimationFrame(verifyRaf);
			if (timeout !== undefined) clearTimeout(timeout);
		}
	};
}

/**
 * Svelte action that persists an element's scrollTop while the page is
 * alive and restores it when the page is mounted again. The restore is
 * retried while the content is still mutating, since async page content
 * can temporarily clamp the scroll position back to 0.
 *
 * Pass `null` to disable the action for an element.
 */
export function rememberScroll(node: HTMLElement, initialKey: string | null = page.url.pathname) {
	let current: { key: string; handle: ScrollMemoryHandle } | null = null;

	const start = (key: string | null, restore: boolean) => {
		current?.handle.stop();
		current = null;

		if (key !== null) {
			current = { key, handle: trackScroll(node, key, restore) };
		}
	};

	start(initialKey, true);

	return {
		update(key: string | null) {
			if (key === (current?.key ?? null)) return;

			// persist the offset under the previous key, but don't restore the
			// new one: its content is already on screen, and jumping it while
			// the user clicks through items would be disorienting
			current?.handle.flush();
			start(key, false);
		},
		destroy() {
			current?.handle.stop();
		}
	};
}
