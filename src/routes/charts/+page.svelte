<script lang="ts">
	// Four charts, all sharing the app's theme and semantic P&L colours.
	import type { EChartsOption } from 'echarts';

	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import { money, num, qty } from '$lib/format';
	import Chart from '$lib/components/Chart.svelte';
	import Card from '$lib/components/ui/Card.svelte';

	// Semantic P&L colours, kept in sync with --profit / --loss in app.css.
	// ECharts needs literal values, so these cannot be CSS variables. The
	// categorical palette for the allocation pie comes from the registered theme
	// (charts/theme.ts) — it must not be duplicated here, or the two would drift.
	const PROFIT = '#22c55e';
	const LOSS = '#ef4444';
	const NEUTRAL = '#60a5fa';

	const pairs = $derived([...new Set(app.trades.map((trade) => trade.pair))].sort());

	let selectedPair = $state('');

	// Default to the pair with the most trades — the one most worth looking at.
	$effect(() => {
		if (selectedPair && pairs.includes(selectedPair)) return;
		if (pairs.length === 0) {
			selectedPair = '';
			return;
		}
		const counts = new Map<string, number>();
		for (const trade of app.trades) {
			counts.set(trade.pair, (counts.get(trade.pair) ?? 0) + 1);
		}
		selectedPair = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? pairs[0]!;
	});

	// --- 1. trade timeline -------------------------------------------------
	const timelineOption = $derived.by((): EChartsOption => {
		const rows = app.trades.filter((trade) => trade.pair === selectedPair);
		const buys = rows
			.filter((trade) => trade.side === 'BUY')
			.map((trade) => [new Date(trade.timestampUtc).getTime(), num(trade.price), num(trade.qty)]);
		const sells = rows
			.filter((trade) => trade.side === 'SELL')
			.map((trade) => [new Date(trade.timestampUtc).getTime(), num(trade.price), num(trade.qty)]);

		return {
			grid: { left: 60, right: 20, top: 30, bottom: 60 },
			legend: { top: 0 },
			tooltip: {
				trigger: 'item',
				formatter: (p: unknown) => {
					const point = p as { seriesName: string; value: [number, number, number] };
					const [time, priceValue, quantity] = point.value;
					return `${point.seriesName}<br/>${new Date(time).toLocaleString()}<br/>${money(String(priceValue))} · ${qty(String(quantity))}`;
				}
			},
			xAxis: { type: 'time' },
			yAxis: { type: 'value', scale: true, splitLine: { lineStyle: { opacity: 0.25 } } },
			// Zoom is essential: a multi-year history is unreadable at full extent.
			dataZoom: [
				{ type: 'inside' },
				{ type: 'slider', height: 22, bottom: 12 }
			],
			series: [
				{
					name: t('charts.buy'),
					type: 'scatter',
					data: buys,
					itemStyle: { color: PROFIT },
					// Marker area scales with trade size, so a large fill stands out.
					symbolSize: (value: number[]) => Math.min(26, 6 + Math.sqrt(value[2] ?? 0) * 3)
				},
				{
					name: t('charts.sell'),
					type: 'scatter',
					data: sells,
					itemStyle: { color: LOSS },
					symbolSize: (value: number[]) => Math.min(26, 6 + Math.sqrt(value[2] ?? 0) * 3)
				}
			]
		};
	});

	// --- 2. cumulative realized P&L ---------------------------------------
	const cumulativeOption = $derived.by((): EChartsOption => {
		let running = 0;
		const points = (app.analysis?.realizedEvents ?? []).map((event) => {
			running += num(event.pnl);
			return [new Date(event.timestamp).getTime(), Number(running.toFixed(8))];
		});

		return {
			grid: { left: 70, right: 20, top: 20, bottom: 40 },
			tooltip: { trigger: 'axis' },
			xAxis: { type: 'time' },
			yAxis: { type: 'value', scale: true, splitLine: { lineStyle: { opacity: 0.25 } } },
			series: [
				{
					type: 'line',
					// Step line, not smooth: P&L changes discretely at each disposal and
					// interpolating between them would imply value that did not exist.
					step: 'end',
					showSymbol: false,
					data: points,
					lineStyle: { width: 2, color: running >= 0 ? PROFIT : LOSS },
					areaStyle: { opacity: 0.12, color: running >= 0 ? PROFIT : LOSS }
				}
			]
		};
	});

	// --- 3. P&L by pair ----------------------------------------------------
	const performanceOption = $derived.by((): EChartsOption => {
		const rows = (app.analysis?.positions ?? [])
			.map((valuation) => ({
				pair: valuation.position.pair,
				pnl: num(valuation.position.realizedPnl) + num(valuation.unrealizedPnl)
			}))
			.filter((row) => row.pnl !== 0)
			.sort((a, b) => a.pnl - b.pnl);

		return {
			grid: { left: 100, right: 30, top: 20, bottom: 30 },
			tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
			xAxis: { type: 'value', splitLine: { lineStyle: { opacity: 0.25 } } },
			yAxis: { type: 'category', data: rows.map((row) => row.pair) },
			series: [
				{
					type: 'bar',
					data: rows.map((row) => ({
						value: Number(row.pnl.toFixed(8)),
						itemStyle: { color: row.pnl >= 0 ? PROFIT : LOSS }
					})),
					barMaxWidth: 22
				}
			]
		};
	});

	// --- 4. allocation -----------------------------------------------------
	const allocationOption = $derived.by((): EChartsOption => {
		const slices = (app.analysis?.positions ?? [])
			.filter((valuation) => num(valuation.marketValueUsdt) > 0)
			.map((valuation) => ({
				name: valuation.position.baseAsset,
				value: num(valuation.marketValueUsdt)
			}))
			.sort((a, b) => b.value - a.value);

		return {
			tooltip: {
				trigger: 'item',
				formatter: (p: unknown) => {
					const point = p as { name: string; value: number; percent: number };
					return `${point.name}<br/>${money(String(point.value))} USDT (${point.percent}%)`;
				}
			},
			legend: { type: 'scroll', orient: 'vertical', right: 8, top: 'middle' },
			series: [
				{
					type: 'pie',
					// Donut rather than full pie: the hole keeps the label ring readable
					// when one holding dominates the book.
					radius: ['45%', '72%'],
					center: ['38%', '50%'],
					itemStyle: { borderWidth: 2, borderColor: 'transparent' },
					label: { show: false },
					data: slices.length > 0 ? slices : [{ name: '—', value: 1, itemStyle: { color: NEUTRAL } }]
				}
			]
		};
	});
</script>

<h1 class="mb-5 text-xl font-semibold tracking-tight">{t('charts.title')}</h1>

{#if !app.hasData}
	<p class="text-sm text-muted-foreground">{t('empty.title')}</p>
{:else}
	<div class="flex flex-col gap-5">
		<Card title={t('charts.timeline')}>
			{#snippet actions()}
				<select
					class="h-8 rounded-md border border-border bg-transparent px-2 text-xs outline-none focus:ring-2 focus:ring-ring"
					bind:value={selectedPair}
					aria-label={t('charts.selectPair')}
				>
					{#each pairs as pair (pair)}<option value={pair}>{pair}</option>{/each}
				</select>
			{/snippet}
			<div class="px-3 pb-3 pt-1">
				<Chart id="timeline" title={t('charts.timeline')} option={timelineOption} height={360} />
			</div>
		</Card>

		<div class="grid grid-cols-1 gap-5 xl:grid-cols-2">
			<Card title={t('charts.cumulativePnl')}>
				<div class="px-3 pb-3 pt-1">
					<Chart
						id="cumulative"
						title={t('charts.cumulativePnl')}
						option={cumulativeOption}
						height={300}
					/>
				</div>
			</Card>

			<Card title={t('charts.coinPerformance')}>
				<div class="px-3 pb-3 pt-1">
					<Chart
						id="performance"
						title={t('charts.coinPerformance')}
						option={performanceOption}
						height={300}
					/>
				</div>
			</Card>
		</div>

		<Card title={t('charts.allocation')}>
			<div class="px-3 pb-3 pt-1">
				<Chart
					id="allocation"
					title={t('charts.allocation')}
					option={allocationOption}
					height={320}
				/>
			</div>
		</Card>
	</div>
{/if}
