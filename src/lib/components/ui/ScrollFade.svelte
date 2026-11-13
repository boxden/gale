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
		let updateFrame = 0;
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
			// the panel caps itself with the floating layer's available-height
			// variable, which follows the final placement; this only drives the
			// fade masks from the actual scroll position
			const maxScroll = actualScroller.scrollHeight - actualScroller.clientHeight;
			const nextTop = actualScroller.scrollTop > 1 ? FADE_SIZE : 0;
			const nextBottom = maxScroll - actualScroller.scrollTop > 1 ? FADE_SIZE : 0;

			if (nextTop === targetTop && nextBottom === targetBottom) return;

			targetTop = nextTop;
			targetBottom = nextBottom;
			animateFade();
		};

		// resize and mutation callbacks can fire many times per frame; coalesce
		// them so the panel is measured and restyled at most once per frame
		const scheduleUpdate = () => {
			if (updateFrame !== 0) return;
			updateFrame = requestAnimationFrame(() => {
				updateFrame = 0;
				update();
			});
		};

		const resizeObserver = new ResizeObserver(scheduleUpdate);
		const mutationObserver = new MutationObserver(scheduleUpdate);

		actualScroller.addEventListener('scroll', scheduleUpdate, { passive: true });
		window.addEventListener('resize', scheduleUpdate);
		window.addEventListener('scroll', scheduleUpdate, true);
		resizeObserver.observe(actualScroller);
		mutationObserver.observe(actualScroller, { childList: true, subtree: true });

		update();

		return () => {
			cancelAnimationFrame(frame);
			cancelAnimationFrame(updateFrame);
			actualScroller.removeEventListener('scroll', scheduleUpdate);
			window.removeEventListener('resize', scheduleUpdate);
			window.removeEventListener('scroll', scheduleUpdate, true);
			resizeObserver.disconnect();
			mutationObserver.disconnect();
			actualScroller.style.maskImage = '';
			actualScroller.style.webkitMaskImage = '';
		};
	});
</script>
