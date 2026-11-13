/**
 * Keep `aria-invalid` in sync with the visual `:user-invalid` state.
 *
 * `:user-invalid` is a CSS-only visual state and is not reflected in the
 * accessibility tree by the browser, so assistive technology would otherwise
 * only learn about a required field being empty if the app set `aria-invalid`
 * itself. Keep it `false` initially so untouched required fields are not
 * announced as invalid, then mirror the pseudo-class after interaction.
 */
export function syncUserInvalid(node: HTMLElement) {
	let supported = true;

	try {
		node.matches(':user-invalid');
	} catch {
		supported = false;
	}

	if (!supported) return;

	const update = () => {
		node.setAttribute('aria-invalid', node.matches(':user-invalid') ? 'true' : 'false');
	};

	node.setAttribute('aria-invalid', 'false');

	const events = ['input', 'change', 'blur', 'focusout'] as const;
	for (const event of events) node.addEventListener(event, update);

	return {
		destroy() {
			for (const event of events) node.removeEventListener(event, update);
		}
	};
}
