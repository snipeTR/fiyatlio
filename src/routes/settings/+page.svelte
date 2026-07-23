<script lang="ts">
	// Settings. Every control writes straight through to the backend, which
	// persists it and returns a recomputed analysis — so switching the P&L method
	// updates every figure in the app immediately, with no separate "apply" step.
	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import { MIN_AUTO_REFRESH_SECS, type Language, type OversellPolicy, type PnlMethod, type ThemeMode } from '$lib/types';
	import Card from '$lib/components/ui/Card.svelte';
	import Field from '$lib/components/ui/Field.svelte';

	const settings = $derived(app.settings);

	const selectClass =
		'h-9 w-full max-w-md rounded-md border border-border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring';
</script>

<h1 class="mb-5 text-xl font-semibold tracking-tight">{t('settings.title')}</h1>

<Card class="max-w-4xl px-5">
	<Field label={t('settings.pnlMethod')} help={t('settings.pnlMethodHelp')}>
		<select
			class={selectClass}
			value={settings.pnlMethod}
			onchange={(event) =>
				app.updateSettings({ pnlMethod: event.currentTarget.value as PnlMethod })}
		>
			<option value="fifo">{t('settings.pnlMethod.fifo')}</option>
			<option value="averageCost">{t('settings.pnlMethod.averageCost')}</option>
		</select>
	</Field>

	<Field label={t('settings.oversellPolicy')} help={t('settings.oversellPolicyHelp')}>
		<select
			class={selectClass}
			value={settings.oversellPolicy}
			onchange={(event) =>
				app.updateSettings({ oversellPolicy: event.currentTarget.value as OversellPolicy })}
		>
			<option value="zeroCostBasis">{t('settings.oversellPolicy.zeroCostBasis')}</option>
			<option value="ignoreExcess">{t('settings.oversellPolicy.ignoreExcess')}</option>
		</select>
	</Field>

	<Field label={t('settings.deduplicate')} help={t('settings.deduplicateHelp')}>
		<label class="inline-flex items-center gap-2 text-sm">
			<input
				type="checkbox"
				class="size-4 accent-current"
				checked={settings.deduplicate}
				onchange={(event) => app.updateSettings({ deduplicate: event.currentTarget.checked })}
			/>
			{settings.deduplicate ? t('common.yes') : t('common.no')}
		</label>
	</Field>

	<Field
		label={t('settings.autoRefresh')}
		help={t('settings.autoRefreshHelp', { min: MIN_AUTO_REFRESH_SECS })}
	>
		<div class="flex flex-wrap items-center gap-4">
			<label class="inline-flex items-center gap-2 text-sm">
				<input
					type="checkbox"
					class="size-4 accent-current"
					checked={settings.autoRefresh}
					onchange={(event) => app.updateSettings({ autoRefresh: event.currentTarget.checked })}
				/>
				{settings.autoRefresh ? t('common.yes') : t('common.no')}
			</label>

			<label class="inline-flex items-center gap-2 text-sm text-muted-foreground">
				{t('settings.autoRefreshSecs')}
				<input
					type="number"
					min={MIN_AUTO_REFRESH_SECS}
					step="10"
					class="num h-9 w-24 rounded-md border border-border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring disabled:opacity-50"
					disabled={!settings.autoRefresh}
					value={settings.autoRefreshSecs}
					onchange={(event) =>
						app.updateSettings({
							// Clamped here as well as in Rust: the number input's own `min`
							// is trivially bypassed by typing, and a 1-second poll would
							// walk straight into Binance's rate limit.
							autoRefreshSecs: Math.max(
								MIN_AUTO_REFRESH_SECS,
								Number(event.currentTarget.value) || MIN_AUTO_REFRESH_SECS
							)
						})}
				/>
			</label>
		</div>
	</Field>

	<Field label={t('settings.language')}>
		<select
			class={selectClass}
			value={settings.language}
			onchange={(event) => app.updateSettings({ language: event.currentTarget.value as Language })}
		>
			<option value="tr">Türkçe</option>
			<option value="en">English</option>
		</select>
	</Field>

	<Field label={t('settings.theme')}>
		<select
			class={selectClass}
			value={settings.theme}
			onchange={(event) => app.updateSettings({ theme: event.currentTarget.value as ThemeMode })}
		>
			<option value="dark">{t('settings.theme.dark')}</option>
			<option value="light">{t('settings.theme.light')}</option>
			<option value="system">{t('settings.theme.system')}</option>
		</select>
	</Field>
</Card>
