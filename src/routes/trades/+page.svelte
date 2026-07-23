<script lang="ts">
	// All Trades: filterable, sortable, virtualised table.
	//
	// Virtualisation is hand-rolled rather than pulled from a library. The rows are
	// fixed height, which reduces the whole problem to slicing the array by scroll
	// offset — a few lines, versus a dependency whose measurement machinery this
	// table does not need. Without it, a 50k-row history would mount 50k DOM rows
	// and lock the window.
	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import { dateTime, money, num, price, qty } from '$lib/format';
	import type { Side, Trade } from '$lib/types';
	import Card from '$lib/components/ui/Card.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import Tooltip from '$lib/components/ui/Tooltip.svelte';

	const ROW_HEIGHT = 36;
	/** Rows rendered above and below the viewport, so fast scrolling stays filled. */
	const OVERSCAN = 8;

	type SortKey = 'time' | 'pair' | 'price' | 'qty' | 'amount';

	let search = $state('');
	let pairFilter = $state('');
	let sideFilter = $state<'' | Side>('');
	let fromDate = $state('');
	let toDate = $state('');

	let sortKey = $state<SortKey>('time');
	let sortAsc = $state(false);

	let viewport = $state<HTMLDivElement | null>(null);
	let scrollTop = $state(0);
	let viewportHeight = $state(600);

	const pairs = $derived([...new Set(app.trades.map((trade) => trade.pair))].sort());

	/** Sell trade ids that the engine flagged as exceeding tracked inventory. */
	const oversoldIds = $derived(
		new Set(
			(app.analysis?.realizedEvents ?? [])
				.filter((event) => event.oversell)
				.map((event) => event.sellTradeId)
		)
	);

	const filtered = $derived.by(() => {
		const needle = search.trim().toLowerCase();
		// Date inputs are calendar days; the upper bound is inclusive, so it is
		// pushed to the end of the chosen day.
		const from = fromDate ? new Date(`${fromDate}T00:00:00`).getTime() : null;
		const to = toDate ? new Date(`${toDate}T23:59:59.999`).getTime() : null;

		return app.trades.filter((trade) => {
			if (pairFilter && trade.pair !== pairFilter) return false;
			if (sideFilter && trade.side !== sideFilter) return false;

			if (from !== null || to !== null) {
				const time = new Date(trade.timestampUtc).getTime();
				if (from !== null && time < from) return false;
				if (to !== null && time > to) return false;
			}

			if (needle) {
				const haystack = `${trade.pair} ${trade.side} ${trade.baseAsset} ${trade.quoteAsset} ${trade.sourceFile}`;
				if (!haystack.toLowerCase().includes(needle)) return false;
			}

			return true;
		});
	});

	const sorted = $derived.by(() => {
		const rows = [...filtered];
		rows.sort((a, b) => {
			let cmp: number;
			switch (sortKey) {
				case 'time':
					cmp = new Date(a.timestampUtc).getTime() - new Date(b.timestampUtc).getTime();
					// Same-second fills keep their original order, matching the engine.
					if (cmp === 0) cmp = a.id - b.id;
					break;
				case 'pair':
					cmp = a.pair.localeCompare(b.pair);
					break;
				case 'price':
					cmp = num(a.price) - num(b.price);
					break;
				case 'qty':
					cmp = num(a.qty) - num(b.qty);
					break;
				case 'amount':
					cmp = num(a.quoteAmount) - num(b.quoteAmount);
					break;
			}
			return sortAsc ? cmp : -cmp;
		});
		return rows;
	});

	const startIndex = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
	const visibleCount = $derived(Math.ceil(viewportHeight / ROW_HEIGHT) + OVERSCAN * 2);
	const visible = $derived(sorted.slice(startIndex, startIndex + visibleCount));

	function toggleSort(key: SortKey) {
		if (sortKey === key) sortAsc = !sortAsc;
		else {
			sortKey = key;
			sortAsc = key === 'pair';
		}
		// Any reorder invalidates the current window; jump back to the top rather
		// than leaving the user parked at an offset that now means nothing.
		viewport?.scrollTo({ top: 0 });
	}

	function onScroll(event: Event) {
		scrollTop = (event.currentTarget as HTMLDivElement).scrollTop;
	}

	// Measure the scroll container once it exists and whenever the row count
	// changes its height. Without this the window size stays at the initial
	// guess, and on a tall display the list would render short of the fold.
	$effect(() => {
		void sorted.length;
		if (viewport) viewportHeight = viewport.clientHeight;
	});

	function feeLabel(trade: Trade): string {
		return num(trade.feeAmount) === 0 ? '—' : `${qty(trade.feeAmount)} ${trade.feeAsset}`;
	}

	const COLUMNS: Array<{ key: SortKey; labelKey: Parameters<typeof t>[0]; align: 'left' | 'right' }> =
		[
			{ key: 'time', labelKey: 'trades.time', align: 'left' },
			{ key: 'pair', labelKey: 'trades.pair', align: 'left' },
			{ key: 'price', labelKey: 'trades.price', align: 'right' },
			{ key: 'qty', labelKey: 'trades.quantity', align: 'right' },
			{ key: 'amount', labelKey: 'trades.amount', align: 'right' }
		];
</script>

<svelte:window onresize={() => (viewportHeight = viewport?.clientHeight ?? viewportHeight)} />

<h1 class="mb-5 text-xl font-semibold tracking-tight">{t('trades.title')}</h1>

{#if !app.hasData}
	<p class="text-sm text-muted-foreground">{t('empty.title')}</p>
{:else}
	<div class="mb-4 flex flex-wrap items-center gap-2">
		<input
			class="h-9 w-56 rounded-md border border-border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
			placeholder={t('trades.search')}
			bind:value={search}
		/>
		<select
			class="h-9 rounded-md border border-border bg-transparent px-2 text-sm outline-none focus:ring-2 focus:ring-ring"
			bind:value={pairFilter}
		>
			<option value="">{t('trades.allPairs')}</option>
			{#each pairs as pair (pair)}<option value={pair}>{pair}</option>{/each}
		</select>
		<select
			class="h-9 rounded-md border border-border bg-transparent px-2 text-sm outline-none focus:ring-2 focus:ring-ring"
			bind:value={sideFilter}
		>
			<option value="">{t('trades.allSides')}</option>
			<option value="BUY">{t('charts.buy')}</option>
			<option value="SELL">{t('charts.sell')}</option>
		</select>
		<label class="flex items-center gap-1.5 text-xs text-muted-foreground">
			{t('trades.from')}
			<input
				type="date"
				class="h-9 rounded-md border border-border bg-transparent px-2 text-sm outline-none focus:ring-2 focus:ring-ring"
				bind:value={fromDate}
			/>
		</label>
		<label class="flex items-center gap-1.5 text-xs text-muted-foreground">
			{t('trades.to')}
			<input
				type="date"
				class="h-9 rounded-md border border-border bg-transparent px-2 text-sm outline-none focus:ring-2 focus:ring-ring"
				bind:value={toDate}
			/>
		</label>
		<span class="ml-auto text-xs text-muted-foreground">
			{t('trades.showing', { shown: sorted.length, total: app.trades.length })}
		</span>
	</div>

	<Card>
		<div class="num sticky top-0 z-10 grid grid-cols-[160px_140px_1fr_1fr_1fr_130px_90px] border-b border-border bg-card text-[11px] uppercase tracking-wider text-muted-foreground">
			{#each COLUMNS as column (column.key)}
				<button
					class="px-4 py-2.5 font-medium hover:text-foreground {column.align === 'left'
						? 'text-left'
						: 'text-right'}"
					onclick={() => toggleSort(column.key)}
				>
					{t(column.labelKey)}
					{#if sortKey === column.key}<span class="ml-1 text-[9px]">{sortAsc ? '▲' : '▼'}</span>{/if}
				</button>
			{/each}
			<div class="px-4 py-2.5 text-right font-medium">{t('trades.fee')}</div>
			<div class="px-4 py-2.5 text-right font-medium">{t('trades.side')}</div>
		</div>

		{#if sorted.length === 0}
			<p class="px-5 py-10 text-center text-sm text-muted-foreground">{t('empty.noTrades')}</p>
		{:else}
			<div
				bind:this={viewport}
				onscroll={onScroll}
				class="relative overflow-y-auto"
				style="height: min(calc(100vh - 320px), {sorted.length * ROW_HEIGHT}px)"
			>
				<!-- Spacer gives the scrollbar the full list's height while only the
				     visible slice is mounted. -->
				<div style="height: {sorted.length * ROW_HEIGHT}px; position: relative;">
					<div style="position: absolute; top: {startIndex * ROW_HEIGHT}px; left: 0; right: 0;">
						{#each visible as trade (trade.id)}
							<div
								class="num grid grid-cols-[160px_140px_1fr_1fr_1fr_130px_90px] items-center border-b border-border/50 text-sm"
								style="height: {ROW_HEIGHT}px"
							>
								<div class="truncate px-4 text-left text-xs text-muted-foreground">
									{dateTime(trade.timestampUtc)}
								</div>
								<div class="flex items-center gap-1.5 px-4 text-left">
									{trade.pair}
									{#if trade.side === 'SELL' && oversoldIds.has(trade.id)}
										<Tooltip text={t('trades.oversellMark')} class="text-warning">
											<Icon name="warning" size={12} />
										</Tooltip>
									{/if}
								</div>
								<div class="px-4 text-right">{price(trade.price)}</div>
								<div class="px-4 text-right">{qty(trade.qty)}</div>
								<div class="px-4 text-right">{money(trade.quoteAmount)}</div>
								<div class="truncate px-4 text-right text-xs text-muted-foreground">
									{feeLabel(trade)}
								</div>
								<div class="px-4 text-right">
									<span
										class="rounded px-1.5 py-0.5 text-[11px] font-medium {trade.side === 'BUY'
											? 'bg-profit/15 text-profit'
											: 'bg-loss/15 text-loss'}"
									>
										{trade.side === 'BUY' ? t('charts.buy') : t('charts.sell')}
									</span>
								</div>
							</div>
						{/each}
					</div>
				</div>
			</div>
		{/if}
	</Card>
{/if}
