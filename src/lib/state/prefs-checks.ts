import { loadIcons } from '@iconify/svelte';
import * as api from '$lib/api';
import { Backend, type Dependant } from '$lib/types';

const tokenStates = new Map<Backend, boolean>();
const tokenRequests = new Map<Backend, Promise<boolean>>();
let hiddenMods: Dependant[] | null = null;
let hiddenModsRequest: Promise<Dependant[]> | null = null;

export function cachedTokenState(backend: Backend) {
	return tokenStates.get(backend) ?? null;
}

export function refreshTokenState(backend: Backend) {
	let request = tokenRequests.get(backend);

	if (!request) {
		request = api.thunderstore
			.hasToken(backend)
			.then((hasToken) => {
				tokenStates.set(backend, hasToken);
				return hasToken;
			})
			.finally(() => tokenRequests.delete(backend));
		tokenRequests.set(backend, request);
	}

	return request;
}

export function cachedHiddenMods() {
	return hiddenMods;
}

export function refreshHiddenMods() {
	hiddenModsRequest ??= api.profile
		.getHiddenMods()
		.then((mods) => (hiddenMods = mods))
		.finally(() => (hiddenModsRequest = null));

	return hiddenModsRequest;
}

export function setHiddenMods(mods: Dependant[]) {
	hiddenMods = mods;
}

// prefetch at startup so the settings tab renders complete rows on its first visit
for (const backend of Object.values(Backend)) {
	refreshTokenState(backend).catch(() => {});
}
refreshHiddenMods().catch(() => {});

// icons resolve asynchronously; warm the settings tab's icons so its rows don't reflow on first open
loadIcons([
	'mdi:question-mark-circle',
	'mdi:folder',
	'mdi:file',
	'mdi:close',
	'mdi:refresh',
	'mdi:chevron-down',
	'mdi:check',
	'mdi:search',
	'mdi:key',
	'mdi:key-remove',
	'mdi:eye-off',
	'mdi:eye',
	'mdi:edit',
	'mdi:monitor',
	'mdi:cog'
]);
