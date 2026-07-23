<script lang="ts">
	// Portfolio table: open positions, then closed ones.
	//
	// A position that is flat but has realized P&L is deliberately kept — dropping
	// it would silently erase completed trading history from the view.
	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import { dateOnly, money, num, price, pnlClass, qty, signedMoney } from '$lib/format';
	import type { PositionValuation } from '$lib/types';
	import Card from '$lib/components/ui/Card.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import Tooltip from '$lib/components/ui/Tooltip.svelte';

	type SortKey = 'pair' | 'qty' | 'value' | 'realized' | 'unrealized' | 'total';

	let sortKey = $state<SortKey>('value');
	let sortAsc = $state(false);
	let expanded = $state<string | null>(null);

	const positions = $derived(app.analysis?.positions ?? []);
	const open = $derived(positions.filter((p) => num(p.position.qty) > 0));
	const closed = $derived(positions.filter((p) => num(p.position.qty) <= 0));

	function totalPnl(v: PositionValuation): number {
		return num(v.position.realizedPnl) + num(v.unrealizedPnl);
	}

	function sortValue(v: PositionValuation, key: SortKey): number | string {
		switch (key) {
			case 'pair':
				return v.position.pair;
			case 'qty':
				return num(v.position.qty);
			case 'value':
				return num(v.marketValueQuote);
			case 'realized':
				return num(v.position.realizedPnl);
			case 'unrealized':
				return num(v.unrealizedPnl);
			case 'total':
				return totalPnl(v);
		}
	}

	const sortedOpen = $derived(
		[...open].sort((a, b) => {
			const left = sortValue(a, sortKey);
			const right = sortValue(b, sortKey);
			const cmp =
				typeof left === 'string' && typeof right === 'string'
					? left.localeCompare(right)
					: Number(left) - Number(right);
			return sortAsc ? cmp : -cmp;
		})
	);

	function toggleSort(key: SortKey) {
		if (sortKey === key) sortAsc = !sortAsc;
		else {
			sortKey = key;
			// Text sorts naturally ascending; numbers are most useful largest-first.
			sortAsc = key === 'pair';
		}
	}

	/** Percentage return on the still-open cost basis. */
	function totalPnlPercent(v: PositionValuation): string {
		const cost = num(v.position.totalCost);
		if (cost === 0) return '—';
		return `${((totalPnl(v) / cost) * 100).toFixed(1)}%`;
	}

	const COLUMNS: Array<{ key: SortKey; labelKey: Parameters<typeof t>[0]; align: 'left' | 'right' }> =
		[
			{ key: 'pair', labelKey: 'portfolio.pair', align: 'left' },
			{ key: 'qty', labelKey: 'portfolio.amount', align: 'right' },
			{ key: 'value', labelKey: 'portfolio.marketValue', align: 'right' },
			{ key: 'realized', labelKey: 'portfolio.realizedPnl', align: 'right' },
			{ key: 'unrealized', labelKey: 'portfolio.unrealizedPnl', align: 'right' },
			{ key: 'total', labelKey: 'portfolio.totalPnl', align: 'right' }
		];
</script>

<h1 class="mb-5 text-xl font-semibold tracking-tight">{t('portfolio.title')}</h1>

{#if !app.hasData}
	<p class="text-sm text-muted-foreground">{t('empty.title')}</p>
{:else}
	<Card title={t('portfolio.open')}>
		{#if sortedOpen.length === 0}
			<p class="px-5 py-8 text-center text-sm text-muted-foreground">{t('empty.noPositions')}</p>
		{:else}
			<div class="overflow-x-auto">
				<table class="num w-full text-sm">
					<thead>
						<tr class="border-b border-border text-[11px] uppercase tracking-wider text-muted-foreground">
							<th class="w-8"></th>
							{#each COLUMNS as column (column.key)}
								<th class="px-4 py-2.5 font-medium {column.align === 'left' ? 'text-left' : 'text-right'}">
									<button
										class="inline-flex items-center gap-1 hover:text-foreground"
										onclick={() => toggleSort(column.key)}
									>
										{t(column.labelKey)}
										{#if sortKey === column.key}
											<span class="text-[9px]">{sortAsc ? '▲' : '▼'}</span>
										{/if}
									</button>
								</th>
							{/each}
							<th class="px-4 py-2.5 text-right font-medium">{t('portfolio.avgCost')}</th>
							<th class="px-4 py-2.5 text-right font-medium">{t('portfolio.currentPrice')}</th>
						</tr>
					</thead>
					<tbody>
						{#each sortedOpen as valuation (valuation.position.pair)}
							{@const p = valuation.position}
							<tr class="border-b border-border/60">
								<td class="pl-3">
									{#if p.openLots.length > 0}
										<button
											class="text-muted-foreground transition-transform hover:text-foreground"
											class:rotate-90={expanded === p.pair}
											onclick={() => (expanded = expanded === p.pair ? null : p.pair)}
											aria-label={t('portfolio.lots')}
										>
											<Icon name="chevronRight" size={14} />
										</button>
									{/if}
								</td>
								<td class="px-4 py-2.5 text-left">
									<span class="inline-flex items-center gap-1.5">
										<span class="font-medium">{p.baseAsset}</span>
										<span class="text-xs text-muted-foreground">/{p.quoteAsset}</span>
										{#if p.hasOversell}
											<Tooltip text={t('trades.oversellMark')} class="text-warning">
												<Icon name="warning" size={13} />
											</Tooltip>
										{/if}
										{#if !valuation.convertibleToUsdt}
											<Tooltip
												text={t('portfolio.notConvertible')}
												class="rounded bg-warning/15 px-1.5 py-0.5 text-[10px] text-warning"
											>
												{p.quoteAsset}
											</Tooltip>
										{/if}
									</span>
								</td>
								<td class="px-4 py-2.5 text-right">{qty(p.qty)}</td>
								<td class="px-4 py-2.5 text-right">
									{money(valuation.marketValueQuote)}
									<span class="text-xs text-muted-foreground">{p.quoteAsset}</span>
								</td>
								<td class="px-4 py-2.5 text-right {pnlClass(p.realizedPnl)}">
									{signedMoney(p.realizedPnl)}
								</td>
								<td class="px-4 py-2.5 text-right {pnlClass(valuation.unrealizedPnl)}">
									{signedMoney(valuation.unrealizedPnl)}
								</td>
								<td class="px-4 py-2.5 text-right {pnlClass(String(totalPnl(valuation)))}">
									{signedMoney(String(totalPnl(valuation)))}
									<span class="ml-1 text-xs opacity-70">{totalPnlPercent(valuation)}</span>
								</td>
								<td class="px-4 py-2.5 text-right text-muted-foreground">{price(p.avgCost)}</td>
								<td class="px-4 py-2.5 text-right">{price(valuation.currentPrice)}</td>
							</tr>

							{#if expanded === p.pair}
								<tr class="bg-accent/20">
									<td colspan="9" class="px-12 py-3">
										<div class="mb-1.5 text-[11px] uppercase tracking-wider text-muted-foreground">
											{t('portfolio.lots')}
										</div>
										<table class="num w-full text-xs">
											<tbody>
												{#each p.openLots as lot (lot.tradeId)}
													<tr class="border-b border-border/40 last:border-b-0">
														<td class="py-1.5 text-left text-muted-foreground">
															{dateOnly(lot.acquiredAt)}
														</td>
														<td class="py-1.5 text-right">{qty(lot.qtyRemaining)} {p.baseAsset}</td>
														<td class="py-1.5 text-right">
															{price(lot.unitCostQuote)} {p.quoteAsset}
														</td>
														<td class="py-1.5 text-right text-muted-foreground">
															{money(lot.costRemaining)} {p.quoteAsset}
														</td>
													</tr>
												{/each}
											</tbody>
										</table>
									</td>
								</tr>
							{/if}
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</Card>

	{#if closed.length > 0}
		<Card class="mt-6" title={t('portfolio.closed')}>
			<div class="overflow-x-auto">
				<table class="num w-full text-sm">
					<thead>
						<tr class="border-b border-border text-[11px] uppercase tracking-wider text-muted-foreground">
							<th class="px-5 py-2.5 text-left font-medium">{t('portfolio.pair')}</th>
							<th class="px-5 py-2.5 text-right font-medium">{t('trades.title')}</th>
							<th class="px-5 py-2.5 text-right font-medium">{t('portfolio.realizedPnl')}</th>
						</tr>
					</thead>
					<tbody>
						{#each closed as valuation (valuation.position.pair)}
							{@const p = valuation.position}
							<tr class="border-b border-border/60 last:border-b-0">
								<td class="px-5 py-2.5 text-left">{p.pair}</td>
								<td class="px-5 py-2.5 text-right text-muted-foreground">
									{p.buyCount + p.sellCount}
								</td>
								<td class="px-5 py-2.5 text-right {pnlClass(p.realizedPnl)}">
									{signedMoney(p.realizedPnl)} {p.quoteAsset}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</Card>
	{/if}
{/if}
