<script lang="ts">
	import type { HTMLTextareaAttributes } from 'svelte/elements';
	import { rememberScroll } from '$lib/scrollMemory';

	type Props = {
		value: string;
		mono?: boolean;
		scrollKey?: string;
	} & HTMLTextareaAttributes;

	let { value = $bindable(), mono = false, scrollKey, ...restProps }: Props = $props();
</script>

<textarea
	bind:value
	use:rememberScroll={scrollKey ?? null}
	{...restProps}
	autocomplete="off"
	class="valid:focus:ring-accent-600 text-primary-700 placeholder-primary-500 hover:border-primary-400 dark:valid:focus:ring-accent-400 dark:bg-primary-900 dark:text-primary-300 dark:placeholder-primary-400 dark:hover:border-primary-500 bg-primary-100 h-40 min-h-11 w-full rounded-lg border border-transparent px-3 py-2 invalid:ring-red-500 focus:border-transparent focus:ring-2 focus:outline-hidden"
	class:font-mono={mono}
></textarea>
