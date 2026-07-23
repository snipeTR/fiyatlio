<script lang="ts">
	// src/routes/+layout.svelte
	// Application shell: sidebar, header, global warnings, toast rail.
	import '../app.css';
	import { onMount } from 'svelte';

	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import { num, qty, signedMoney } from '$lib/format';
	import FileDrop from '$lib/components/FileDrop.svelte';
	import PriceStatus from '$lib/components/PriceStatus.svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import Toasts from '$lib/components/Toasts.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import Tooltip from '$lib/components/ui/Tooltip.svelte';

	let { children } = $props();

	const summary = $derived(app.analysis?.summary ?? null);

	// Assets whose disposals had no recorded acquisition, largest first — the
	// list tells the user exactly which coins to look for in their Convert or
	// transfer history.
	const oversellAssets = $derived(
		Object.entries(summary?.oversellQtyByAsset ?? {})
			.sort((a, b) => num(b[1]) - num(a[1]))
			.slice(0, 8)
			.map(([asset, amount]) => `${qty(amount)} ${asset}`)
			.join(', ')
	);

	onMount(() => {
		void app.init();

		// The OS theme can change while the app is open; only follow it when the
		// user actually chose "system".
		const media = window.matchMedia('(prefers-color-scheme: dark)');
		const onSchemeChange = () => {
			if (app.settings.theme === 'system') app.applyTheme('system');
		};
		media.addEventListener('change', onSchemeChange);

		return () => {
			media.removeEventListener('change', onSchemeChange);
			app.teardown();
		};
	});
</script>

<div class="flex h-screen overflow-hidden bg-background text-foreground">
	<Sidebar />

	<div class="flex min-w-0 flex-1 flex-col">
		<header class="flex h-14 shrink-0 items-center justify-between gap-4 border-b border-border px-6">
			<div class="flex items-center gap-3">
				<FileDrop compact />
				{#if app.hasData}
					<Tooltip text={t('action.clear')}>
						<Button variant="ghost" size="sm" onclick={() => app.clearData()}>
							<Icon name="trash" size={14} />
						</Button>
					</Tooltip>
				{/if}
			</div>
			<PriceStatus />
		</header>

		<!-- Persistent, non-dismissible advisories: these change how the numbers
		     below should be read, so they must not be something the user can
		     scroll past and forget. -->
		{#if summary?.hasOversell}
			<div class="border-b border-warning/30 bg-warning/10 px-6 py-3 text-xs text-warning">
				<div class="flex items-start gap-2">
					<span class="mt-px shrink-0"><Icon name="warning" size={14} /></span>
					<div class="min-w-0 flex-1 space-y-1">
						<!-- Quantified, not just flagged: the whole point is to let the
						     user see how much of the headline profit is an artifact of
						     acquisitions the CSV cannot show. -->
						<p class="font-medium">
							{t('warning.oversellDetail', {
								count: summary.oversellEventCount,
								pnl: signedMoney(summary.oversellPnlUsdt)
							})}
						</p>
						<p class="opacity-80">{t('warning.oversellCause')}</p>
						{#if oversellAssets}
							<p class="num opacity-80">
								{t('warning.oversellAssets', { assets: oversellAssets })}
							</p>
						{/if}
					</div>
					{#if app.settings.oversellPolicy === 'zeroCostBasis'}
						<Button
							variant="outline"
							size="sm"
							class="shrink-0"
							onclick={() => app.updateSettings({ oversellPolicy: 'ignoreExcess' })}
						>
							{t('warning.oversellSwitch')}
						</Button>
					{/if}
				</div>
			</div>
		{/if}

		{#if summary && summary.excludedPairs.length > 0}
			<div class="flex items-start gap-2 border-b border-warning/30 bg-warning/10 px-6 py-2.5 text-xs text-warning">
				<span class="mt-px shrink-0"><Icon name="warning" size={14} /></span>
				<span>{t('warning.excludedPairs', { pairs: summary.excludedPairs.join(', ') })}</span>
			</div>
		{/if}

		{#if app.restorableFiles.length > 0}
			<div class="flex items-center justify-between gap-4 border-b border-border bg-accent/40 px-6 py-2.5 text-xs">
				<span>{t('import.restoreSession', { count: app.restorableFiles.length })}</span>
				<div class="flex gap-2">
					<Button size="sm" onclick={() => app.importFiles(app.restorableFiles)}>
						{t('action.restore')}
					</Button>
					<Button variant="ghost" size="sm" onclick={() => (app.restorableFiles = [])}>
						{t('action.dismiss')}
					</Button>
				</div>
			</div>
		{/if}

		<main class="min-w-0 flex-1 overflow-y-auto px-6 py-6">
			{#if !app.initialised}
				<p class="text-sm text-muted-foreground">{t('common.loading')}</p>
			{:else}
				{@render children?.()}
			{/if}
		</main>
	</div>
</div>

<Toasts />
