// the tallest the popup can get, matching the max-h-96 cap on the panels
const MAX_PANEL_HEIGHT = 24 * 16;
// rough row pitch, only used to judge which side has room for the list
const ROW_HEIGHT = 30;
const PANEL_PADDING = 8;
const WINDOW_MARGIN = 8;

/**
 * Picks the side of the trigger a popup should open on. The popup clamps its
 * own height to the floating layer's available space on that side, so the list
 * stays inside the window even when it is very short.
 */
export function popupSide(
	trigger: HTMLElement | null | undefined,
	itemCount: number
): 'top' | 'bottom' {
	if (!trigger) return 'bottom';

	const rect = trigger.getBoundingClientRect();
	const below = window.innerHeight - rect.bottom - WINDOW_MARGIN;
	const above = rect.top - WINDOW_MARGIN;
	const needed = Math.min(MAX_PANEL_HEIGHT, itemCount * ROW_HEIGHT + PANEL_PADDING);

	// keep the natural downward direction while the list fits there; otherwise
	// open towards the side with more room
	return below >= needed || below >= above ? 'bottom' : 'top';
}
