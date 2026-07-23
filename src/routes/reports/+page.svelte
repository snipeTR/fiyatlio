<script lang="ts">
	// Reports and exports.
	//
	// PDF works by opening the generated HTML in its own window and invoking the
	// system print dialog. That is the only cross-platform route that does not
	// require bundling a headless browser, and it gives the user the OS's own
	// "Save as PDF" with page setup, margins and preview already handled.
	import { save } from '@tauri-apps/plugin-dialog';
	import { openPath } from '@tauri-apps/plugin-opener';

	import { captureCharts } from '$lib/charts/registry';
	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import { api } from '$lib/api';
	import type { ExportFormat, ExportKind } from '$lib/types';
	import Button from '$lib/components/ui/Button.svelte';
	import Card from '$lib/components/ui/Card.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';

	let includeCharts = $state(true);
	let busy = $state(false);
	let lastReportPath = $state<string | null>(null);

	async function generate(): Promise<string | null> {
		if (!app.hasData) {
			app.toast('error', 'error.data.none');
			return null;
		}

		busy = true;
		try {
			const suggested = await api.suggestedReportFilename();
			const path = await save({
				defaultPath: suggested,
				filters: [{ name: 'HTML', extensions: ['html'] }]
			});
			if (!path) return null;

			// Charts are only capturable while the Charts page has them mounted.
			// If the user has not visited it this session the report is generated
			// without images rather than failing.
			const charts = includeCharts ? captureCharts() : [];

			const written = await api.generateReport(path, {
				language: app.settings.language,
				charts
			});
			lastReportPath = written;
			app.toast('success', 'reports.generated', { path: written });
			return written;
		} catch (error) {
			app.reportError(error);
			return null;
		} finally {
			busy = false;
		}
	}

	async function openReport() {
		const path = lastReportPath ?? (await generate());
		if (!path) return;
		try {
			await openPath(path);
		} catch (error) {
			app.reportError(error);
		}
	}

	async function exportData(kind: ExportKind, format: ExportFormat) {
		if (!app.hasData) {
			app.toast('error', 'error.data.none');
			return;
		}

		busy = true;
		try {
			const suggested = await api.suggestedExportFilename(kind, format);
			const path = await save({
				defaultPath: suggested,
				filters: [{ name: format.toUpperCase(), extensions: [format] }]
			});
			if (!path) return;

			const written = await api.exportData(kind, format, path);
			app.toast('success', 'reports.exported', { path: written });
		} catch (error) {
			app.reportError(error);
		} finally {
			busy = false;
		}
	}

	const chartCount = $derived(includeCharts ? captureCharts().length : 0);
</script>

<h1 class="mb-5 text-xl font-semibold tracking-tight">{t('reports.title')}</h1>

<div class="flex max-w-4xl flex-col gap-5">
	<Card title={t('action.generateReport')}>
		<div class="px-5 py-4">
			<p class="text-sm leading-relaxed text-muted-foreground">{t('reports.description')}</p>

			<label class="mt-4 inline-flex items-center gap-2 text-sm">
				<input type="checkbox" class="size-4 accent-current" bind:checked={includeCharts} />
				{t('reports.includeCharts')}
				{#if includeCharts}
					<span class="text-xs text-muted-foreground">({chartCount})</span>
				{/if}
			</label>

			<div class="mt-5 flex flex-wrap gap-2">
				<Button onclick={generate} disabled={busy || !app.hasData}>
					<Icon name="report" size={15} />
					{t('action.generateReport')}
				</Button>
				<Button variant="outline" onclick={openReport} disabled={busy || !app.hasData}>
					<Icon name="print" size={15} />
					{t('action.savePdf')}
				</Button>
			</div>

			<p class="mt-3 text-xs leading-relaxed text-muted-foreground">{t('reports.pdfHint')}</p>

			{#if lastReportPath}
				<p class="num mt-3 break-all rounded-md bg-accent/40 px-3 py-2 text-xs text-muted-foreground">
					{lastReportPath}
				</p>
			{/if}
		</div>
	</Card>

	<Card title={t('action.export')}>
		<div class="flex flex-col gap-4 px-5 py-4">
			<div class="flex flex-wrap items-center gap-2">
				<span class="w-56 text-sm">{t('reports.exportTrades')}</span>
				<Button
					variant="outline"
					size="sm"
					onclick={() => exportData('trades', 'csv')}
					disabled={busy || !app.hasData}
				>
					<Icon name="download" size={14} /> CSV
				</Button>
				<Button
					variant="outline"
					size="sm"
					onclick={() => exportData('trades', 'json')}
					disabled={busy || !app.hasData}
				>
					<Icon name="download" size={14} /> JSON
				</Button>
			</div>

			<div class="flex flex-wrap items-center gap-2">
				<span class="w-56 text-sm">{t('reports.exportEvents')}</span>
				<Button
					variant="outline"
					size="sm"
					onclick={() => exportData('realizedEvents', 'csv')}
					disabled={busy || !app.hasData}
				>
					<Icon name="download" size={14} /> CSV
				</Button>
				<Button
					variant="outline"
					size="sm"
					onclick={() => exportData('realizedEvents', 'json')}
					disabled={busy || !app.hasData}
				>
					<Icon name="download" size={14} /> JSON
				</Button>
			</div>
		</div>
	</Card>

	{#if (app.analysis?.import.errors.length ?? 0) > 0}
		<Card title={t('import.errorList')}>
			<div class="max-h-80 overflow-y-auto">
				<table class="num w-full text-xs">
					<thead>
						<tr class="border-b border-border text-[10px] uppercase tracking-wider text-muted-foreground">
							<th class="px-5 py-2 text-left font-medium">{t('import.row')}</th>
							<th class="px-5 py-2 text-left font-medium">{t('import.reason')}</th>
							<th class="px-5 py-2 text-left font-medium">{t('trades.source')}</th>
						</tr>
					</thead>
					<tbody>
						{#each app.analysis?.import.errors ?? [] as rowError (`${rowError.fileIndex}-${rowError.rowIndex}`)}
							<tr class="border-b border-border/50 last:border-b-0">
								<td class="px-5 py-1.5 text-left text-muted-foreground">{rowError.rowIndex}</td>
								<td class="px-5 py-1.5 text-left">
									{t(rowError.reasonKey as Parameters<typeof t>[0])}
									<span class="ml-2 text-muted-foreground">{rowError.detail}</span>
								</td>
								<td class="truncate px-5 py-1.5 text-left text-muted-foreground" title={rowError.file}>
									{rowError.file.split(/[\\/]/).pop()}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</Card>
	{/if}
</div>
