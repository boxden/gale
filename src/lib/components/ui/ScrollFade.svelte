<script lang="ts">
	type Props = {
		scroller?: HTMLElement | undefined;
		container?: HTMLElement | undefined;
	};

	let { scroller, container }: Props = $props();

	const FADE_SIZE = 24;
	const FADE_DURATION = 200;

	$effect(() => {
		const panel = container ?? scroller;
		if (!panel) return;

		// select panels scroll their viewport, not the bordered panel itself
		const actualScroller =
			scroller ?? panel.querySelector<HTMLElement>('[data-select-viewport]') ?? panel;

		let frame = 0;
		let currentTop = 0;
		let currentBottom = 0;
		let targetTop = 0;
		let targetBottom = 0;

		const applyMask = () => {
			if (currentTop < 0.01 && currentBottom < 0.01) {
				actualScroller.style.maskImage = '';
				actualScroller.style.webkitMaskImage = '';
				return;
			}

			const mask = `linear-gradient(to bottom, transparent 0, black ${currentTop}px, black calc(100% - ${currentBottom}px), transparent 100%)`;

			actualScroller.style.maskImage = mask;
			actualScroller.style.webkitMaskImage = mask;
		};

		const animateFade = () => {
			cancelAnimationFrame(frame);

			const fromTop = currentTop;
			const fromBottom = currentBottom;
			const start = performance.now();

			const step = (now: number) => {
				const progress = Math.min(1, (now - start) / FADE_DURATION);
				const eased = 1 - Math.pow(1 - progress, 3);

				currentTop = fromTop + (targetTop - fromTop) * eased;
				currentBottom = fromBottom + (targetBottom - fromBottom) * eased;
				applyMask();

				if (progress < 1) {
					frame = requestAnimationFrame(step);
				} else {
					frame = 0;
				}
			};

			frame = requestAnimationFrame(step);
		};

		const update = () => {
			// the floating layer only exposes its available height through a CSS
			// variable, which is not always resolved; cap the panel from the
			// viewport directly so it always stays inside the window
			const viewportRoom = window.innerHeight - panel.getBoundingClientRect().top - 8;
			panel.style.maxHeight = `${Math.min(24 * 16, Math.max(0, viewportRoom))}px`;

			const maxScroll = actualScroller.scrollHeight - actualScroller.clientHeight;
			const nextTop = actualScroller.scrollTop > 1 ? FADE_SIZE : 0;
			const nextBottom = maxScroll - actualScroller.scrollTop > 1 ? FADE_SIZE : 0;

			if (nextTop === targetTop && nextBottom === targetBottom) return;

			targetTop = nextTop;
			targetBottom = nextBottom;
			animateFade();
		};

		const resizeObserver = new ResizeObserver(update);
		const mutationObserver = new MutationObserver(update);

		actualScroller.addEventListener('scroll', update, { passive: true });
		window.addEventListener('resize', update);
		window.addEventListener('scroll', update, true);
		resizeObserver.observe(actualScroller);
		mutationObserver.observe(actualScroller, { childList: true, subtree: true });

		update();

		return () => {
			cancelAnimationFrame(frame);
			actualScroller.removeEventListener('scroll', update);
			window.removeEventListener('resize', update);
			window.removeEventListener('scroll', update, true);
			resizeObserver.disconnect();
			mutationObserver.disconnect();
			actualScroller.style.maskImage = '';
			actualScroller.style.webkitMaskImage = '';
		};
	});
</script>
