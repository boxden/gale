import * as api from '$lib/api';
import type { Prefs } from '$lib/types';

let cached: Prefs | null = null;
let pending: Promise<Prefs> | null = null;

async function fetchPrefs() {
	const prefs = await api.prefs.get();
	prefs.gamePrefs = new Map(Object.entries(prefs.gamePrefs));

	cached = prefs;
	return prefs;
}

/** Last snapshot, so the settings tab can render before its own fetch returns. */
export function cachedPrefs() {
	return cached;
}

/** Coalesces concurrent callers into one request. */
export function refreshPrefs() {
	pending ??= fetchPrefs().finally(() => (pending = null));
	return pending;
}

// prefetch at startup so the first visit to the settings tab renders from cache
refreshPrefs().catch(() => {});
