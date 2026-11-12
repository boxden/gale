<script lang="ts">
	import Info from '$lib/components/ui/Info.svelte';
	import Label from '$lib/components/ui/Label.svelte';
	import Icon from '@iconify/svelte';
	import { untrack } from 'svelte';
	import { apiKeyDialog } from '$lib/state/misc.svelte';
	import { cachedTokenState, refreshTokenState } from '$lib/state/prefs-checks';
	import type { Backend } from '$lib/types';
	import { m } from '$lib/paraglide/messages';

	type Props = {
		backend: Backend;
	};

	let { backend }: Props = $props();

	let hasToken: boolean | null = $state(untrack(() => cachedTokenState(backend)));

	function openDialog() {
		apiKeyDialog.open = true;
		apiKeyDialog.backend = backend;
	}

	$effect(() => {
		apiKeyDialog.open;
		apiKeyDialog.backend;
		refreshTokenState(backend)
			.then((state) => (hasToken = state))
			.catch(() => {});
	});
</script>

<div class="flex items-center">
	<Label>{m.apiKeyPref_title({ backend })}</Label>

	<Info>
		{m.apiKeyPref_content({ backend })}
	</Info>

	<button
		class="group hover:border-primary-400 dark:bg-primary-900 dark:hover:border-primary-500 bg-primary-100 relative flex grow items-center truncate rounded-lg border border-transparent px-3 py-1 text-right"
		onclick={openDialog}
	>
		<div class="mr-2 rounded-sm">
			<Icon
				icon={hasToken === false ? 'mdi:key-remove' : 'mdi:key'}
				class="text-primary-600 group-hover:text-primary-700 dark:text-primary-300 dark:group-hover:text-primary-200 align-middle"
			/>
		</div>

		<div
			class="text-primary-600 group-hover:text-primary-700 dark:text-primary-300 dark:group-hover:text-primary-200 truncate"
		>
			{hasToken === null ? '\u200b' : m[`apiKeyPref_hasToken_${hasToken ? 'has' : 'no'}`]()}
		</div>
	</button>
</div>
