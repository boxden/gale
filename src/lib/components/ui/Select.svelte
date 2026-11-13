<script lang="ts">
	import { dropIn, dropOut } from '$lib/transitions';
	import { popupSide } from '$lib/popup';
	import { emptyOrUndefined } from '$lib/util';
	import Icon from '@iconify/svelte';
	import { Select, type WithoutChildren } from 'bits-ui';

	import type { Snippet } from 'svelte';
	import { fade, fly } from 'svelte/transition';
	import DropdownArrow from './DropdownArrow.svelte';
	import ScrollFade from './ScrollFade.svelte';

	type Props = WithoutChildren<Select.RootProps> & {
		placeholder?: string;
		items: { value: string; label: string; description?: string; disabled?: boolean }[];
		triggerClass?: string;
		icon?: string;
		item?: Snippet<[{ label: string; value: string; selected: boolean }]>;
	} & (
			| {
					label?: never;
			  }
			| {
					placeholder?: never;
					label: Snippet<[{ defaultLabel: string | null }]>;
			  }
			| {
					placeholder?: never;
					label: string;
			  }
		);

	let content = $state<HTMLDivElement>();
	let trigger = $state<HTMLElement | null>(null);
	let side = $state<'top' | 'bottom'>('bottom');

	let {
		open = $bindable(false),
		value = $bindable(),
		triggerClass,
		items,
		placeholder,
		icon,
		label,
		item: itemSnippet,
		...restProps
	}: Props = $props();

	// measured before the panel mounts so the first placement already opens
	// towards the side that has room for the list
	$effect.pre(() => {
		if (open) side = popupSide(trigger, items.length);
	});

	// the window can resize or the page can scroll under an open panel, so the
	// side keeps following the free space instead of staying where it opened
	$effect(() => {
		if (!open) return;

		let frame = 0;
		const update = () => {
			cancelAnimationFrame(frame);
			frame = requestAnimationFrame(() => (side = popupSide(trigger, items.length)));
		};

		window.addEventListener('resize', update);
		window.addEventListener('scroll', update, true);

		return () => {
			cancelAnimationFrame(frame);
			window.removeEventListener('resize', update);
			window.removeEventListener('scroll', update, true);
		};
	});

	const selectedLabel = $derived(
		restProps.type === 'single'
			? items.find((item) => item.value === value)?.label
			: emptyOrUndefined(
					items
						.filter((item) => value?.includes(item.value))
						.map((item) => item.label)
						.join(', ')
				)
	);

	// bits-ui centres the selected item before the floating layer has placed
	// the panel and before the scroll fade clamps its height, so the offset is
	// computed against a viewport that is still changing. Re-centre once the
	// panel settles; the user's first gesture ends the adjustment.
	$effect(() => {
		if (!open || !content) return;

		const viewport = content.querySelector<HTMLElement>('[data-select-viewport]');
		if (!viewport) return;

		const center = () => {
			const item = viewport.querySelector<HTMLElement>('[data-selected]');
			if (!item) return;

			viewport.scrollTop = Math.max(
				0,
				item.offsetTop + item.offsetHeight / 2 - viewport.clientHeight / 2
			);
		};

		let frames = 0;
		let frame = requestAnimationFrame(function step() {
			center();
			if (frames++ < 3) frame = requestAnimationFrame(step);
		});

		const observer = new ResizeObserver(center);
		observer.observe(viewport);

		let timer: ReturnType<typeof setTimeout> | undefined;
		const stop = () => {
			cancelAnimationFrame(frame);
			if (timer !== undefined) clearTimeout(timer);
			observer.disconnect();
			viewport.removeEventListener('wheel', stop);
			viewport.removeEventListener('touchstart', stop);
			viewport.removeEventListener('pointerdown', stop);
		};
		timer = setTimeout(stop, 300);

		viewport.addEventListener('wheel', stop, { passive: true });
		viewport.addEventListener('touchstart', stop, { passive: true });
		viewport.addEventListener('pointerdown', stop);

		return stop;
	});
</script>

<Select.Root bind:value={value as never} bind:open scrollAlignment="center" {...restProps}>
	<Select.Trigger
		bind:ref={trigger}
		class={[
			triggerClass,
			'group enabled:hover:border-primary-400 dark:bg-primary-900 dark:enabled:hover:border-primary-500 bg-primary-100 flex items-center gap-2 overflow-hidden rounded-lg border border-transparent py-1 pr-2 pl-3'
		]}
	>
		{#if icon}
			<Icon class="text-primary-500 dark:text-primary-400 shrink-0 text-lg" {icon} />
		{/if}

		{#if label && typeof label !== 'string'}
			{@render label({ defaultLabel: selectedLabel ?? null })}
		{:else}
			<div
				class={[
					label || selectedLabel
						? 'text-primary-700 dark:text-primary-300'
						: 'text-primary-500 dark:text-primary-400',
					'group-disabled:text-primary-500 dark:group-disabled:text-primary-400 shrink grow overflow-hidden text-left text-nowrap'
				]}
			>
				{label ?? selectedLabel ?? placeholder}
			</div>
		{/if}

		<DropdownArrow
			{open}
			class="text-primary-500 group-disabled:text-primary-500 dark:text-primary-400 dark:group-disabled:text-primary-500 ml-auto"
		/>
	</Select.Trigger>
	<Select.Portal>
		<Select.Content
			forceMount
			{side}
			avoidCollisions={false}
			collisionPadding={8}
			preventOverflowTextSelection={false}
		>
			{#snippet child({ wrapperProps, props, open })}
				<div {...wrapperProps}>
					{#if open}
						<div class="relative">
							<div
								bind:this={content}
								{...props}
								class="select-panel border-primary-300 dark:border-primary-600 dark:bg-primary-800 z-40 flex w-(--bits-select-anchor-width) gap-0.5 overflow-y-auto rounded-lg border bg-white p-1 shadow-xl"
								in:fly={dropIn}
								out:fade={dropOut}
							>
								<Select.Viewport>
									{#each items as item, i (i + item.value)}
										{@const { description, ...itemProps } = item}
										<Select.Item
											{...itemProps}
											class="hover:text-primary-800 group dark:hover:bg-primary-700 dark:hover:text-primary-200 hover:bg-primary-200 flex w-full cursor-default items-center rounded-md px-3 py-1"
										>
											{#snippet children({ selected })}
												{#if itemSnippet}
													{@render itemSnippet({ ...item, selected })}
												{/if}

												<div class="flex grow flex-col overflow-hidden">
													<span
														class={[
															'overflow-hidden text-nowrap',
															selected
																? 'text-primary-700 dark:text-primary-300'
																: 'text-primary-500 group-hover:text-primary-700 dark:text-primary-400 dark:group-hover:text-primary-300'
														]}>{item.label}</span
													>

													{#if description}
														<span class="text-primary-500 dark:text-primary-400 text-xs">
															{description}
														</span>
													{/if}
												</div>

												{#if selected}
													<Icon
														icon="mdi:check"
														class="text-accent-600 dark:text-accent-400 ml-auto shrink-0 text-lg"
													/>
												{/if}
											{/snippet}
										</Select.Item>
									{/each}
								</Select.Viewport>
							</div>

							<ScrollFade container={content} />
						</div>
					{/if}
				</div>
			{/snippet}
		</Select.Content>
	</Select.Portal>
</Select.Root>

<style>
	/* the floating layer reports the space left on the chosen side, so the panel
	   never extends past the window edge, even when the window is very short */
	.select-panel {
		max-height: min(24rem, var(--bits-select-content-available-height, 24rem));
	}
</style>
