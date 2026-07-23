<script lang="ts">
	// Price freshness indicator.
	//
	// Offline-first means the app keeps showing the last known prices rather than
	// blanking out — which is only honest if the UI says so. Three states: live,
	// stale (restored from cache or last fetch failed), and never fetched.
	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import { dateTime, relativeTime } from '$lib/format';
	import Button from '$lib/components/ui/Button.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import Tooltip from '$lib/components/ui/Tooltip.svelte';

	const snapshot = $derived(app.analysis?.summary.priceSnapshot ?? null);
</script>

<div class="flex items-center gap-3">
	{#if snapshot}
		<Tooltip
			text={dateTime(snapshot.fetchedAt)}
			class="flex items-center gap-1.5 text-xs {snapshot.stale
				? 'text-warning'
				: 'text-muted-foreground'}"
		>
			{#if snapshot.stale}
				<Icon name="offline" size={14} />
				{t('price.stale')}
			{:else}
				<span class="inline-block size-1.5 rounded-full bg-profit"></span>
				{t('price.live')}
			{/if}
			<span class="opacity-70">· {t('price.asOf', { time: relativeTime(snapshot.fetchedAt) })}</span>
		</Tooltip>
	{:else}
		<span class="flex items-center gap-1.5 text-xs text-muted-foreground">
			<Icon name="offline" size={14} />
			{t('price.never')}
		</span>
	{/if}

	<Button
		variant="outline"
		size="sm"
		onclick={() => app.refreshPrices()}
		disabled={app.refreshing || !app.hasData}
	>
		<Icon name="refresh" size={14} class={app.refreshing ? 'animate-spin' : ''} />
		{t('action.refresh')}
	</Button>
</div>
