<script lang="ts">
	// src/routes/+page.svelte
	//
	// Phase 0 smoke screen: proves the SvelteKit ⇄ Tauri IPC bridge works before
	// any real feature exists. Replaced by the Dashboard in Phase 3.
	import { invoke } from '@tauri-apps/api/core';

	let version = $state<string | null>(null);
	let ipcError = $state<string | null>(null);

	$effect(() => {
		invoke<string>('app_version')
			.then((v) => {
				version = v;
			})
			.catch((e: unknown) => {
				ipcError = typeof e === 'string' ? e : JSON.stringify(e);
			});
	});
</script>

<main class="flex min-h-screen flex-col items-center justify-center gap-4 p-8">
	<h1 class="text-3xl font-semibold tracking-tight">Fiyatlio</h1>
	<p class="text-muted-foreground">Binance Spot Trading Analiz</p>

	{#if version}
		<p class="num rounded-md border border-border px-3 py-1 text-sm">
			IPC OK — v{version}
		</p>
	{:else if ipcError}
		<p class="rounded-md border border-destructive px-3 py-1 text-sm text-destructive">
			IPC hatası: {ipcError}
		</p>
	{:else}
		<p class="text-sm text-muted-foreground">IPC kontrol ediliyor…</p>
	{/if}

	<p class="mt-6 text-xs text-muted-foreground">Faz 0 — iskelet</p>
</main>
