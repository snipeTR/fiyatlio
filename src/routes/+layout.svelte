<script lang="ts">
	// src/routes/+layout.svelte
	// Phase 0: root shell only. The sidebar navigation, i18n provider and theme
	// controller land in Phase 3.
	import '../app.css';

	let { children } = $props();
</script>

<div class="min-h-screen bg-background text-foreground">
	{@render children?.()}
</div>
