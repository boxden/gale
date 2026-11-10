<script lang="ts">
	import type { HTMLTextareaAttributes } from 'svelte/elements';
	import { rememberScroll } from '$lib/scrollMemory';

	type Props = {
		value: string;
		mono?: boolean;
		scrollKey?: string;
	} & HTMLTextareaAttributes;

	let { value = $bindable(), mono = false, scrollKey, ...restProps }: Props = $props();

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
		const observer = new ResizeObserver(() => {
			if (raf !== 0) return;
			raf = requestAnimationFrame(() => {
				raf = 0;
				if (!node.style.width && !node.style.height) return;

				sessionStorage.setItem(
					storageKey,
					JSON.stringify({ width: node.style.width, height: node.style.height })
				);
			});
		});
		observer.observe(node);

		return {
			destroy() {
				observer.disconnect();
				if (raf !== 0) cancelAnimationFrame(raf);
			}
		};
	}
</script>

<textarea
	bind:value
	use:rememberScroll={scrollKey ?? null}
	use:rememberSize={scrollKey}
	{...restProps}
	autocomplete="off"
	class="valid:focus:ring-accent-600 text-primary-700 placeholder-primary-500 hover:border-primary-400 dark:valid:focus:ring-accent-400 dark:bg-primary-900 dark:text-primary-300 dark:placeholder-primary-400 dark:hover:border-primary-500 bg-primary-100 h-40 max-w-full min-h-11 w-full rounded-lg border border-transparent px-3 py-2 invalid:ring-red-500 focus:border-transparent focus:ring-2 focus:outline-hidden"
	class:font-mono={mono}
></textarea>
