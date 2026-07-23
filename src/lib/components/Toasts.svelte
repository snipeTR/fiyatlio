<script lang="ts">
	// Global toast rail.
	//
	// Errors persist until dismissed (see `AppStore.toast`) because a financial
	// figure that failed to load is not something to let scroll past. Their
	// technical detail sits behind a disclosure so the surface message stays
	// translated and readable.
	import { app } from '$lib/stores/app.svelte';
	import { t, type TranslationKey } from '$lib/i18n/index.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';

	let expanded = $state<number | null>(null);
</script>

<div class="pointer-events-none fixed bottom-4 right-4 z-50 flex w-96 max-w-[calc(100vw-2rem)] flex-col gap-2">
	{#each app.toasts as toast (toast.id)}
		<div
			class="pointer-events-auto rounded-lg border px-4 py-3 shadow-lg backdrop-blur
				{toast.kind === 'error'
				? 'border-destructive/40 bg-destructive/10 text-destructive'
				: toast.kind === 'success'
					? 'border-profit/40 bg-profit/10 text-profit'
					: 'border-border bg-card text-card-foreground'}"
			role={toast.kind === 'error' ? 'alert' : 'status'}
		>
			<div class="flex items-start gap-2.5">
				<span class="mt-0.5 shrink-0">
					<Icon
						name={toast.kind === 'error' ? 'warning' : toast.kind === 'success' ? 'check' : 'info'}
						size={16}
					/>
				</span>
				<div class="min-w-0 flex-1 text-sm">
					<p class="leading-snug">{t(toast.messageKey as TranslationKey, toast.params)}</p>

					{#if toast.detail}
						<button
							class="mt-1 text-xs underline decoration-dotted underline-offset-2 opacity-70 hover:opacity-100"
							onclick={() => (expanded = expanded === toast.id ? null : toast.id)}
						>
							{expanded === toast.id ? t('action.hideDetails') : t('action.showDetails')}
						</button>
						{#if expanded === toast.id}
							<pre class="mt-1.5 max-h-40 overflow-auto whitespace-pre-wrap break-all rounded bg-background/60 p-2 text-[11px] leading-relaxed opacity-80">{toast.detail}</pre>
						{/if}
					{/if}
				</div>
				<button
					class="shrink-0 opacity-60 transition-opacity hover:opacity-100"
					onclick={() => app.dismissToast(toast.id)}
					aria-label={t('action.close')}
				>
					<Icon name="close" size={15} />
				</button>
			</div>
		</div>
	{/each}
</div>
