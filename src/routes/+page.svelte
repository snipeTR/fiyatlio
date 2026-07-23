<script lang="ts">
	// Dashboard.
	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import { dateOnly, dateTime, money, num, percent, pnlClass, qty, signedMoney } from '$lib/format';
	import FileDrop from '$lib/components/FileDrop.svelte';
	import Card from '$lib/components/ui/Card.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import StatCard from '$lib/components/ui/StatCard.svelte';
	import Tooltip from '$lib/components/ui/Tooltip.svelte';

	const analysis = $derived(app.analysis);
	const summary = $derived(analysis?.summary ?? null);

	// Newest first, capped at ten — the dashboard is a glance, not the log.
	const recentEvents = $derived([...(analysis?.realizedEvents ?? [])].reverse().slice(0, 10));

	// Full per-asset breakdown lives in the tooltip; the card itself only says how
	// many assets are involved, because listing one of twenty-six is misleading.
	const feeBreakdown = $derived(
		Object.entries(summary?.totalFeesByAsset ?? {})
			.sort((a, b) => num(b[1]) - num(a[1]))
			.map(([asset, amount]) => `${qty(amount)} ${asset}`)
			.join('\n')
	);
	const feeAssetCount = $derived(Object.keys(summary?.totalFeesByAsset ?? {}).length);
</script>

<h1 class="mb-5 text-xl font-semibold tracking-tight">{t('dashboard.title')}</h1>

{#if !app.hasData}
	<div class="mx-auto max-w-2xl">
		<div class="mb-6 text-center">
			<h2 class="text-lg font-medium">{t('empty.title')}</h2>
			<p class="mx-auto mt-2 max-w-md text-sm text-muted-foreground">{t('empty.body')}</p>
		</div>
		<FileDrop />
	</div>
{:else if summary && analysis}
	<div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
		<StatCard
			label={t('dashboard.realizedPnl')}
			value="{signedMoney(summary.totalRealizedPnlUsdt)} USDT"
			valueClass={pnlClass(summary.totalRealizedPnlUsdt)}
		/>
		<StatCard
			label={t('dashboard.unrealizedPnl')}
			value="{signedMoney(summary.totalUnrealizedPnlUsdt)} USDT"
			valueClass={pnlClass(summary.totalUnrealizedPnlUsdt)}
		/>
		<StatCard
			label={t('dashboard.portfolioValue')}
			value="{money(summary.totalPortfolioValueUsdt)} USDT"
		/>
		<StatCard
			label={t('dashboard.winRate')}
			value={percent(summary.winRate)}
			sub="{summary.winningEvents} {t('common.of')} {summary.totalSellEvents}"
		/>
		<StatCard label={t('dashboard.totalTrades')} value={String(summary.totalTrades)} />
		<StatCard
			label={t('dashboard.totalFees')}
			value="{t('common.approx')} {money(summary.totalFeesUsdtApprox)} USDT"
			sub={feeAssetCount > 0 ? t('dashboard.feeAssets', { count: feeAssetCount }) : undefined}
			tooltip={`${t('dashboard.feeTooltip')}\n\n${feeBreakdown}`}
		/>
	</div>

	<div class="mt-4 flex flex-wrap gap-x-6 gap-y-1 text-xs text-muted-foreground">
		<span>
			{t('import.dateRange')}:
			{dateOnly(analysis.import.firstTradeAt)} – {dateOnly(analysis.import.lastTradeAt)}
		</span>
		<span>{t('import.pairs')}: {analysis.import.pairs.length}</span>
		<span>{t('import.validRows')}: {analysis.import.validRows}</span>
		{#if analysis.import.invalidRows > 0}
			<span class="text-warning">{t('import.invalidRows')}: {analysis.import.invalidRows}</span>
		{/if}
		{#if analysis.import.duplicateRows > 0}
			<span>{t('import.duplicateRows')}: {analysis.import.duplicateRows}</span>
		{/if}
	</div>

	<Card class="mt-6" title={t('dashboard.recentEvents')}>
		{#if recentEvents.length === 0}
			<p class="px-5 py-8 text-center text-sm text-muted-foreground">{t('empty.noEvents')}</p>
		{:else}
			<div class="overflow-x-auto">
				<table class="num w-full text-sm">
					<thead>
						<tr class="border-b border-border text-[11px] uppercase tracking-wider text-muted-foreground">
							<th class="px-5 py-2.5 text-left font-medium">{t('trades.time')}</th>
							<th class="px-5 py-2.5 text-left font-medium">{t('trades.pair')}</th>
							<th class="px-5 py-2.5 text-right font-medium">{t('trades.quantity')}</th>
							<th class="px-5 py-2.5 text-right font-medium">{t('dashboard.proceeds')}</th>
							<th class="px-5 py-2.5 text-right font-medium">{t('charts.pnl')}</th>
						</tr>
					</thead>
					<tbody>
						{#each recentEvents as event (event.sellTradeId)}
							<tr class="border-b border-border/60 last:border-b-0">
								<td class="px-5 py-2.5 text-left text-muted-foreground">
									{dateTime(event.timestamp)}
								</td>
								<td class="px-5 py-2.5 text-left">
									<span class="inline-flex items-center gap-1.5">
										{event.pair}
										{#if event.oversell}
											<Tooltip text={t('trades.oversellMark')} class="text-warning">
												<Icon name="warning" size={13} />
											</Tooltip>
										{/if}
									</span>
								</td>
								<td class="px-5 py-2.5 text-right">{qty(event.qty)}</td>
								<td class="px-5 py-2.5 text-right text-muted-foreground">
									{money(event.proceeds)} {event.quoteAsset}
								</td>
								<td class="px-5 py-2.5 text-right {pnlClass(event.pnl)}">
									{signedMoney(event.pnl)} {event.quoteAsset}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</Card>
{/if}
