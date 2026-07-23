<script lang="ts">
	import { cn } from '$lib/utils';
	import type { Snippet } from 'svelte';

	type Variant = 'default' | 'outline' | 'ghost' | 'destructive';
	type Size = 'sm' | 'md';

	let {
		variant = 'default',
		size = 'md',
		type = 'button',
		disabled = false,
		title,
		class: className = '',
		onclick,
		children
	}: {
		variant?: Variant;
		size?: Size;
		type?: 'button' | 'submit';
		disabled?: boolean;
		title?: string;
		class?: string;
		onclick?: (event: MouseEvent) => void;
		children?: Snippet;
	} = $props();

	const VARIANTS: Record<Variant, string> = {
		default: 'bg-primary text-primary-foreground hover:opacity-90',
		outline: 'border border-border bg-transparent hover:bg-accent hover:text-accent-foreground',
		ghost: 'bg-transparent hover:bg-accent hover:text-accent-foreground',
		destructive: 'bg-destructive text-destructive-foreground hover:opacity-90'
	};

	const SIZES: Record<Size, string> = {
		sm: 'h-8 px-3 text-xs',
		md: 'h-9 px-4 text-sm'
	};
</script>

<button
	{type}
	{disabled}
	{title}
	{onclick}
	class={cn(
		'inline-flex items-center justify-center gap-2 rounded-md font-medium transition-colors',
		'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
		'disabled:pointer-events-none disabled:opacity-50',
		VARIANTS[variant],
		SIZES[size],
		className
	)}
>
	{@render children?.()}
</button>
