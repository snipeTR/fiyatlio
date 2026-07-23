<script lang="ts">
	import { cn } from '$lib/utils';
	import type { Snippet } from 'svelte';

	let {
		class: className = '',
		title,
		subtitle,
		actions,
		children
	}: {
		class?: string;
		title?: string;
		subtitle?: string;
		actions?: Snippet;
		children?: Snippet;
	} = $props();
</script>

<section
	class={cn('rounded-xl border border-border bg-card text-card-foreground shadow-sm', className)}
>
	{#if title || actions}
		<header class="flex items-start justify-between gap-4 border-b border-border px-5 py-4">
			<div>
				{#if title}<h2 class="text-sm font-semibold tracking-tight">{title}</h2>{/if}
				{#if subtitle}<p class="mt-1 text-xs text-muted-foreground">{subtitle}</p>{/if}
			</div>
			{#if actions}
				<div class="flex shrink-0 items-center gap-2">{@render actions()}</div>
			{/if}
		</header>
	{/if}
	{@render children?.()}
</section>
