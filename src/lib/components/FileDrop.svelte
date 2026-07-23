<script lang="ts">
	// CSV intake: native file picker plus drag & drop.
	//
	// Drag & drop uses Tauri's window-level `onDragDropEvent` rather than the DOM
	// drag events. In a Tauri webview the DOM drop gives a `File` object with no
	// real filesystem path, and the backend needs a path to read — the Tauri event
	// is the only source of one.
	import { getCurrentWebview } from '@tauri-apps/api/webview';
	import { open } from '@tauri-apps/plugin-dialog';
	import { onMount } from 'svelte';

	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';

	let { compact = false }: { compact?: boolean } = $props();

	let dragging = $state(false);

	function csvOnly(paths: string[]): string[] {
		return paths.filter((path) => path.toLowerCase().endsWith('.csv'));
	}

	async function pickFiles() {
		try {
			const selection = await open({
				multiple: true,
				filters: [{ name: 'CSV', extensions: ['csv'] }]
			});
			if (!selection) return;

			const paths = Array.isArray(selection) ? selection : [selection];
			await app.importFiles(csvOnly(paths));
		} catch (error) {
			app.reportError(error);
		}
	}

	onMount(() => {
		// `onDragDropEvent` resolves to an unlisten function; the promise means we
		// cannot return it directly from onMount, so it is stored and called in the
		// cleanup. Without this, navigating away and back would stack listeners.
		let unlisten: (() => void) | null = null;
		let disposed = false;

		void getCurrentWebview()
			.onDragDropEvent((event) => {
				if (event.payload.type === 'over') {
					dragging = true;
				} else if (event.payload.type === 'drop') {
					dragging = false;
					const paths = csvOnly(event.payload.paths);
					if (paths.length > 0) void app.importFiles(paths);
				} else {
					dragging = false;
				}
			})
			.then((fn) => {
				if (disposed) fn();
				else unlisten = fn;
			})
			.catch((error) => app.reportError(error));

		return () => {
			disposed = true;
			unlisten?.();
		};
	});
</script>

{#if compact}
	<Button variant="outline" size="sm" onclick={pickFiles} disabled={app.importing}>
		<Icon name="upload" size={15} />
		{app.importing ? t('common.loading') : t('action.selectFiles')}
	</Button>
{:else}
	<div
		class="flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed px-8 py-14 text-center transition-colors
			{dragging ? 'border-primary bg-accent/40' : 'border-border bg-card/40'}"
	>
		<span class="text-muted-foreground"><Icon name="upload" size={28} /></span>
		<div>
			<p class="text-sm font-medium">{t('drop.title')}</p>
			<p class="mt-1 text-xs text-muted-foreground">{t('empty.hint')}</p>
		</div>
		<div class="flex items-center gap-3 text-xs text-muted-foreground">
			<span>{t('drop.or')}</span>
			<Button onclick={pickFiles} disabled={app.importing}>
				<Icon name="upload" size={15} />
				{app.importing ? t('common.loading') : t('action.selectFiles')}
			</Button>
		</div>
	</div>
{/if}
