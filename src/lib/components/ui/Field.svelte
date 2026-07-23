<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		label,
		help,
		children
	}: { label: string; help?: string; children?: Snippet } = $props();
</script>

<div class="flex flex-col gap-2 border-b border-border py-5 last:border-b-0 sm:flex-row sm:items-start sm:gap-8">
	<div class="sm:w-72 sm:shrink-0">
		<div class="text-sm font-medium">{label}</div>
		{#if help}
			<p class="mt-1 text-xs leading-relaxed text-muted-foreground">{help}</p>
		{/if}
	</div>
	<div class="flex-1">{@render children?.()}</div>
</div>
