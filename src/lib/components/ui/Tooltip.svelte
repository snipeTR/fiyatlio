<script lang="ts">
	// Hover/focus tooltip.
	//
	// Replaces the native `title` attribute. Three reasons the native one was not
	// good enough here: WebView2 shows it only after a long, unpredictable delay
	// (and not at all in some window states, which is what made it look broken);
	// it cannot render multi-line text reliably; and it is invisible to keyboard
	// users, who never hover.
	//
	// Positioned with `position: fixed` against the trigger's viewport rect, so it
	// escapes any `overflow: hidden` ancestor — table cells and cards clip an
	// absolutely positioned child.
	import type { Snippet } from 'svelte';

	let {
		text,
		placement = 'bottom',
		class: className = '',
		children
	}: {
		text: string;
		placement?: 'top' | 'bottom';
		class?: string;
		children?: Snippet;
	} = $props();

	let open = $state(false);
	let trigger = $state<HTMLSpanElement | null>(null);
	let x = $state(0);
	let y = $state(0);

	const GAP = 8;

	function show() {
		if (!trigger || !text) return;

		const rect = trigger.getBoundingClientRect();
		x = rect.left + rect.width / 2;
		y = placement === 'top' ? rect.top - GAP : rect.bottom + GAP;
		open = true;
	}

	function hide() {
		open = false;
	}
</script>

<span
	bind:this={trigger}
	class={className}
	role="button"
	tabindex="0"
	onmouseenter={show}
	onmouseleave={hide}
	onfocusin={show}
	onfocusout={hide}
	onkeydown={(event) => {
		if (event.key === 'Escape') hide();
	}}
>
	{@render children?.()}
</span>

{#if open && text}
	<div
		role="tooltip"
		class="pointer-events-none fixed z-[100] max-w-xs whitespace-pre-line rounded-md border border-border
			bg-popover px-2.5 py-1.5 text-xs leading-relaxed text-popover-foreground shadow-xl"
		style="left: {x}px; top: {y}px; transform: translate(-50%, {placement === 'top'
			? '-100%'
			: '0'});"
	>
		{text}
	</div>
{/if}
