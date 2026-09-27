// src/lib/api.ts
//
// Typed wrappers around every Tauri command, plus the single place where backend
// errors are normalised.
//
// The backend serialises `AppError` as `{ kind, messageKey, detail }`. Anything
// that does not match that shape (a thrown string from the IPC layer itself, an
// exception in a plugin) is coerced into the same shape here, so callers only
// ever handle one error type.

import { invoke } from '@tauri-apps/api/core';

import {
	isAppError,
	type AnalysisResult,
	type AppErrorPayload,
	type ExportFormat,
	type ExportKind,
	type ReportOptions,
	type Settings,
	type Trade
} from '$lib/types';

/** Error type every caller in the app deals with. */
export class ApiError extends Error {
	readonly kind: string;
	readonly messageKey: string;
	readonly detail: string;
	readonly path: string;
	readonly options: { id: string; labelKey: string }[];

	constructor(payload: AppErrorPayload) {
		super(payload.detail || payload.messageKey);
		this.name = 'ApiError';
		this.kind = payload.kind;
		this.messageKey = payload.messageKey;
		this.detail = payload.detail;
		this.path = payload.path ?? '';
		this.options = payload.options ?? [];
	}
}

function normalise(error: unknown): ApiError {
	if (error instanceof ApiError) return error;
	if (isAppError(error)) return new ApiError(error);

	// The IPC layer itself failed, or a plugin threw. There is no messageKey to
	// map, so fall back to the generic internal error and keep the raw text as
	// detail rather than discarding the only diagnostic we have.
	return new ApiError({
		kind: 'internal',
		messageKey: 'error.internal',
		detail: typeof error === 'string' ? error : JSON.stringify(error)
	});
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	try {
		return await invoke<T>(command, args);
	} catch (error) {
		throw normalise(error);
	}
}

export const api = {
	appVersion: () => call<string>('app_version'),

	getSettings: () => call<Settings>('get_settings'),
	setSettings: (settings: Settings) => call<AnalysisResult>('set_settings', { settings }),

	getAnalysis: () => call<AnalysisResult>('get_analysis'),
	getTrades: () => call<Trade[]>('get_trades'),
	importCsvFiles: (paths: string[], formatOverrides: Record<string, string> = {}) =>
		call<AnalysisResult>('import_csv_files', { paths, formatOverrides }),
	clearData: () => call<AnalysisResult>('clear_data'),
	refreshPrices: () => call<AnalysisResult>('refresh_prices'),

	suggestedExportFilename: (kind: ExportKind, format: ExportFormat) =>
		call<string>('suggested_export_filename', { kind, format }),
	exportData: (kind: ExportKind, format: ExportFormat, path: string) =>
		call<string>('export_data', { kind, format, path }),

	suggestedReportFilename: () => call<string>('suggested_report_filename'),
	generateReport: (path: string, options: ReportOptions) =>
		call<string>('generate_report', { path, options })
};
