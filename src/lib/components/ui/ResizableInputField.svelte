<script lang="ts">
	import type { HTMLTextareaAttributes } from 'svelte/elements';
	import { rememberScroll } from '$lib/scrollMemory';

	type Props = {
		value: string;
		mono?: boolean;
		scrollKey?: string;
	} & HTMLTextareaAttributes;

	let { value = $bindable(), mono = false, scrollKey, ...restProps }: Props = $props();

	let textarea: HTMLTextAreaElement;

	function startResize(evt: PointerEvent) {
		if (evt.button !== 0) return;

		const grip = evt.currentTarget as HTMLElement;
		const node = textarea;
		const startY = evt.clientY;
		const startHeight = node.offsetHeight;
		const minHeight = parseFloat(getComputedStyle(node).minHeight) || 0;

		grip.setPointerCapture(evt.pointerId);
		document.body.style.userSelect = 'none';

		const onMove = (move: PointerEvent) => {
			node.style.height = `${Math.max(minHeight, startHeight + move.clientY - startY)}px`;
		};

		const onEnd = () => {
			grip.removeEventListener('pointermove', onMove);
			grip.removeEventListener('pointerup', onEnd);
			grip.removeEventListener('pointercancel', onEnd);
			document.body.style.userSelect = '';
		};

		grip.addEventListener('pointermove', onMove);
		grip.addEventListener('pointerup', onEnd);
		grip.addEventListener('pointercancel', onEnd);
	}

	// collapsing the field shrinks the page and clamps its scroll position, so
	// the scroll offsets are restored once the field has its size back
	function saveScrollOffsets(node: HTMLElement) {
		const offsets: { element: HTMLElement; top: number }[] = [
			{ element: node, top: node.scrollTop }
		];

		for (let element = node.parentElement; element; element = element.parentElement) {
			const { overflowY } = getComputedStyle(element);
			if (
				(overflowY === 'auto' || overflowY === 'scroll') &&
				element.scrollHeight > element.clientHeight
			) {
				offsets.push({ element, top: element.scrollTop });
			}
		}

		return offsets;
	}

	function fitToContent() {
		const node = textarea;
		const border = node.offsetHeight - node.clientHeight;
		const offsets = saveScrollOffsets(node);
		const currentHeight = node.offsetHeight;

		// measure the default height without the inline height applied
		const previousHeight = node.style.height;
		node.style.height = '';
		const defaultHeight = node.offsetHeight;
		node.style.height = previousHeight;

		if (Math.abs(currentHeight - defaultHeight) > 1) {
			// any other size collapses back to the default height
			node.style.height = '';
			node.style.height = getComputedStyle(node).height;
		} else {
			// at the default height, expand to the content but keep the bottom
			// handle reachable: no taller than 70% of the viewport, with an
			// internal scrollbar beyond that
			const maxHeight = window.innerHeight * 0.7;
			let contentHeight;

			if (node.scrollHeight > node.clientHeight) {
				// overflowing content can be measured without collapsing the field
				contentHeight = node.scrollHeight + border;
			} else {
				// the field is taller than its content, so collapse it to measure
				node.style.height = '0px';
				contentHeight = node.scrollHeight + border;
			}

			const targetHeight = Math.max(defaultHeight, Math.min(contentHeight, maxHeight));

			node.style.height = `${targetHeight}px`;
		}

		for (const { element, top } of offsets) element.scrollTop = top;
	}

	// a drag-resize becomes inline width/height; saving that pair rather than
	// the laid-out box keeps the chosen size even when the parent clamps the
	// rendered width, and sessionStorage scopes the memory to this session
	function rememberSize(node: HTMLTextAreaElement, key: string | undefined) {
		if (key === undefined) return;

		const storageKey = `textarea-size:${key}`;

		try {
			const saved = JSON.parse(sessionStorage.getItem(storageKey) ?? 'null') as {
				width?: string;
				height?: string;
			} | null;

			if (saved?.width) node.style.width = saved.width;
			if (saved?.height) node.style.height = saved.height;
		} catch {
			sessionStorage.removeItem(storageKey);
		}

		let raf = 0;
		let saveTimer: ReturnType<typeof setTimeout> | undefined;

		const save = () => {
			saveTimer = undefined;
			if (!node.style.width && !node.style.height) return;

			sessionStorage.setItem(
				storageKey,
				JSON.stringify({ width: node.style.width, height: node.style.height })
			);
		};

		const observer = new ResizeObserver(() => {
			if (raf !== 0) return;
			raf = requestAnimationFrame(() => {
				raf = 0;
				if (!node.style.width && !node.style.height) return;

				// a window resize changes the laid-out size on every frame; only
				// persist the chosen size once the resize settles
				if (saveTimer !== undefined) clearTimeout(saveTimer);
				saveTimer = setTimeout(save, 150);
			});
		});
		observer.observe(node);

		return {
			destroy() {
				observer.disconnect();
				if (raf !== 0) cancelAnimationFrame(raf);
				if (saveTimer !== undefined) {
					clearTimeout(saveTimer);
					save();
				}
			}
		};
	}
</script>

<div class="relative w-full">
	<!-- clips the scrollbar corner to the rounded field outline -->
	<div
		class="has-[textarea:valid]:focus-within:ring-accent-600 dark:has-[textarea:valid]:focus-within:ring-accent-400 inline-block w-full overflow-hidden rounded-lg focus-within:ring-2 has-[textarea:user-invalid]:ring-1 has-[textarea:user-invalid]:ring-red-500 has-[textarea:user-invalid]:focus-within:ring-2"
	>
		<textarea
			bind:this={textarea}
			bind:value
			use:rememberScroll={scrollKey ?? null}
			use:rememberSize={scrollKey}
			{...restProps}
			autocomplete="off"
			class="text-primary-700 placeholder-primary-500 hover:border-primary-400 dark:bg-primary-900 dark:text-primary-300 dark:placeholder-primary-400 dark:hover:border-primary-500 bg-primary-100 block h-40 min-h-11 w-full max-w-full rounded-lg border border-transparent px-3 py-2 focus:border-transparent focus:outline-hidden"
			class:font-mono={mono}
		></textarea>
	</div>

	<!-- stops before the scrollbars so their corner and the native resizer stay usable -->
	<div
		class="absolute right-5 bottom-0 left-0 h-2 cursor-ns-resize touch-none"
		role="separator"
		aria-orientation="horizontal"
		onpointerdown={startResize}
		ondblclick={fitToContent}
	></div>
</div>
