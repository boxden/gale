import { getLocale, overwriteGetLocale, type Locale } from '$lib/paraglide/runtime';

let current = $state<Locale>(getLocale());

// Route paraglide's getLocale through reactive state so every m.*() call in
// a template subscribes to the locale and re-renders on language change —
// no page reload or remount required.
overwriteGetLocale(() => current);

export function setCurrentLocale(locale: Locale) {
	current = locale;
}
