// src/lib/stores/app.svelte.ts
//
// The single source of UI state, built on Svelte 5 runes.
//
// Design: the backend owns all derived numbers and returns a complete
// `AnalysisResult` from every mutating command. This store therefore never
// recomputes anything financial — it just holds the latest result, the settings,
// and the transient UI concerns (loading flags, toasts). That is what keeps the
// displayed figures and the backend's books from ever drifting apart.

import { api, ApiError } from '$lib/api';
import { i18n } from '$lib/i18n/index.svelte';
import type { AnalysisResult, Language, Settings, ThemeMode, Trade } from '$lib/types';
import { MIN_AUTO_REFRESH_SECS } from '$lib/types';

export interface Toast {
	id: number;
	kind: 'info' | 'success' | 'error';
	/** i18n key. */
	messageKey: string;
	params?: Record<string, string | number>;
	/** Untranslated technical detail, shown behind a disclosure. */
	detail?: string;
}

const DEFAULT_SETTINGS: Settings = {
	pnlMethod: 'fifo',
	oversellPolicy: 'zeroCostBasis',
	deduplicate: true,
	autoRefresh: false,
	autoRefreshSecs: MIN_AUTO_REFRESH_SECS,
	language: 'tr',
	theme: 'dark',
	lastFiles: []
};

class AppStore {
	analysis = $state<AnalysisResult | null>(null);
	/**
	 * Raw trade list. Fetched separately from `analysis` and only when it can
	 * actually have changed (import / clear / startup), because a large history is
	 * tens of thousands of rows and a price refresh does not touch any of them.
	 */
	trades = $state<Trade[]>([]);
	settings = $state<Settings>({ ...DEFAULT_SETTINGS });
	version = $state('');

	importing = $state(false);
	refreshing = $state(false);
	initialised = $state(false);

	toasts = $state<Toast[]>([]);

	/** Offered on startup when the last session had files loaded. */
	restorableFiles = $state<string[]>([]);

	/** Handle for the auto-refresh interval, so it can be cancelled cleanly. */
	#refreshTimer: ReturnType<typeof setInterval> | null = null;
	#toastSeq = 0;

	get hasData(): boolean {
		return (this.analysis?.import.validRows ?? 0) > 0;
	}

	// -- toasts ------------------------------------------------------------

	toast(kind: Toast['kind'], messageKey: string, params?: Toast['params'], detail?: string) {
		const id = ++this.#toastSeq;
		this.toasts = [...this.toasts, { id, kind, messageKey, params, detail }];

		// Errors stay until dismissed; informational toasts fade on their own.
		if (kind !== 'error') {
			setTimeout(() => this.dismissToast(id), 4000);
		}
	}

	dismissToast(id: number) {
		this.toasts = this.toasts.filter((toast) => toast.id !== id);
	}

	/** Funnels any thrown value into a user-visible, translated error toast. */
	reportError(error: unknown) {
		const api_error =
			error instanceof ApiError
				? error
				: new ApiError({
						kind: 'internal',
						messageKey: 'error.internal',
						detail: String(error)
					});

		console.error(api_error);
		this.toast('error', api_error.messageKey, undefined, api_error.detail);
	}

	// -- theme & language --------------------------------------------------

	applyTheme(theme: ThemeMode) {
		if (typeof document === 'undefined') return;

		const prefersDark =
			typeof window !== 'undefined' &&
			window.matchMedia('(prefers-color-scheme: dark)').matches;
		const dark = theme === 'dark' || (theme === 'system' && prefersDark);

		document.documentElement.classList.toggle('dark', dark);
		document.documentElement.dataset.theme = dark ? 'dark' : 'light';
	}

	applyLanguage(language: Language) {
		i18n.set(language);
	}

	// -- lifecycle ---------------------------------------------------------

	async init() {
		if (this.initialised) return;

		try {
			this.version = await api.appVersion();
			this.settings = await api.getSettings();
			this.applyTheme(this.settings.theme);
			this.applyLanguage(this.settings.language);

			this.analysis = await api.getAnalysis();
			this.trades = await api.getTrades();

			// Do not silently re-read last session's files: the user may have moved
			// or deleted them, and re-importing without asking is surprising.
			if (this.settings.lastFiles.length > 0 && !this.hasData) {
				this.restorableFiles = this.settings.lastFiles;
			}

			this.syncAutoRefresh();
		} catch (error) {
			this.reportError(error);
		} finally {
			this.initialised = true;
		}
	}

	// -- data --------------------------------------------------------------

	async importFiles(paths: string[]) {
		if (paths.length === 0) return;

		this.importing = true;
		this.restorableFiles = [];
		try {
			this.analysis = await api.importCsvFiles(paths);
			this.trades = await api.getTrades();
			this.settings = await api.getSettings();

			const skipped = this.analysis.import.invalidRows;
			const duplicates = this.analysis.import.duplicateRows;
			if (skipped > 0) {
				this.toast('info', 'import.skippedWarning', { count: skipped });
			}
			if (duplicates > 0) {
				this.toast('info', 'import.duplicateWarning', { count: duplicates });
			}

			// Fetch prices right after a successful import so the dashboard is not
			// briefly showing cost basis with no market value.
			await this.refreshPrices({ silent: true });
		} catch (error) {
			this.reportError(error);
		} finally {
			this.importing = false;
		}
	}

	async clearData() {
		try {
			this.analysis = await api.clearData();
			this.trades = [];
		} catch (error) {
			this.reportError(error);
		}
	}

	/**
	 * Refreshes prices.
	 *
	 * `silent` suppresses the error toast for background refreshes — the stale
	 * indicator in the header already communicates the state, and a toast every
	 * 30 seconds while offline would be hostile.
	 */
	async refreshPrices({ silent = false }: { silent?: boolean } = {}) {
		if (this.refreshing) return;

		this.refreshing = true;
		try {
			this.analysis = await api.refreshPrices();

			const failed = this.analysis.summary.priceSnapshot?.failedSymbols ?? [];
			if (failed.length > 0 && !silent) {
				this.toast('info', 'price.failedSymbols', { symbols: failed.join(', ') });
			}
		} catch (error) {
			if (!silent) this.reportError(error);
		} finally {
			this.refreshing = false;
		}
	}

	// -- settings ----------------------------------------------------------

	async updateSettings(patch: Partial<Settings>) {
		const next: Settings = { ...this.settings, ...patch };
		if (next.autoRefreshSecs < MIN_AUTO_REFRESH_SECS) {
			next.autoRefreshSecs = MIN_AUTO_REFRESH_SECS;
		}

		// Apply presentation changes immediately — waiting for the round trip
		// makes the theme and language toggles feel broken.
		if (patch.theme) this.applyTheme(next.theme);
		if (patch.language) this.applyLanguage(next.language);

		try {
			this.analysis = await api.setSettings(next);
			this.settings = next;
			this.syncAutoRefresh();
		} catch (error) {
			this.reportError(error);
			// Roll the presentation back so the UI matches what was actually saved.
			this.applyTheme(this.settings.theme);
			this.applyLanguage(this.settings.language);
		}
	}

	/**
	 * Starts or stops the auto-refresh interval to match the current settings.
	 *
	 * Always clears the previous timer first: without that, toggling the setting
	 * repeatedly would leak an interval per toggle and quietly multiply the
	 * request rate against Binance.
	 */
	syncAutoRefresh() {
		if (this.#refreshTimer !== null) {
			clearInterval(this.#refreshTimer);
			this.#refreshTimer = null;
		}

		if (!this.settings.autoRefresh) return;

		const seconds = Math.max(this.settings.autoRefreshSecs, MIN_AUTO_REFRESH_SECS);
		this.#refreshTimer = setInterval(() => {
			void this.refreshPrices({ silent: true });
		}, seconds * 1000);
	}

	/** Called when the app shuts down or the layout unmounts. */
	teardown() {
		if (this.#refreshTimer !== null) {
			clearInterval(this.#refreshTimer);
			this.#refreshTimer = null;
		}
	}
}

export const app = new AppStore();
