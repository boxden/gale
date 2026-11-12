<script module lang="ts">
	import type { Backend } from '$lib/types';

	const knownTokenStates = new Map<Backend, boolean>();
</script>

<script lang="ts">
	import Info from '$lib/components/ui/Info.svelte';
	import Label from '$lib/components/ui/Label.svelte';
	import * as api from '$lib/api';
	import Icon from '@iconify/svelte';
	import type { Snippet } from 'svelte';
	import { untrack } from 'svelte';
	import { apiKeyDialog } from '$lib/state/misc.svelte';
	import { m } from '$lib/paraglide/messages';

	type Props = {
		backend: Backend;
	};

	let { backend }: Props = $props();

	let hasToken: boolean | null = $state(untrack(() => knownTokenStates.get(backend) ?? null));

	async function refresh() {
		hasToken = await api.thunderstore.hasToken(backend);
		knownTokenStates.set(backend, hasToken);
	}

	function openDialog() {
		apiKeyDialog.open = true;
		apiKeyDialog.backend = backend;
	}

	$effect(() => {
		apiKeyDialog.open;
		apiKeyDialog.backend;
		refresh();
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
			{#if hasToken !== null}
				{m[`apiKeyPref_hasToken_${hasToken ? 'has' : 'no'}`]()}
			{/if}
		</div>
	</button>
</div>
