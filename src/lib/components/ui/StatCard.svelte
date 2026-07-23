<script lang="ts">
	import { cn } from '$lib/utils';
	import Icon from './Icon.svelte';
	import Tooltip from './Tooltip.svelte';

	let {
		label,
		value,
		sub,
		valueClass = '',
		tooltip
	}: {
		label: string;
		value: string;
		sub?: string;
		valueClass?: string;
		tooltip?: string;
	} = $props();
</script>

<div class="rounded-xl border border-border bg-card p-4">
	<div class="flex items-center gap-1.5">
		<span class="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
			{label}
		</span>
		{#if tooltip}
			<Tooltip text={tooltip} class="cursor-help text-muted-foreground/70 hover:text-foreground">
				<Icon name="info" size={13} />
			</Tooltip>
		{/if}
	</div>
	<div class={cn('num mt-1.5 text-2xl font-semibold tracking-tight', valueClass)}>{value}</div>
	{#if sub}
		<div class="num mt-0.5 truncate text-xs text-muted-foreground">{sub}</div>
	{/if}
</div>
