// src/lib/format.ts
//
// Display formatting only.
//
// Everything here calls `Number(...)` on a `DecimalString`. That is safe for
// PRESENTATION — a double carries ~15–17 significant digits, comfortably more
// than an 8-decimal crypto quantity needs — but it must never feed a value back
// into a calculation whose result the user acts on. All arithmetic stays in Rust
// where the value is a `Decimal`.

import { i18n } from '$lib/i18n/index.svelte';
import type { DecimalString } from '$lib/types';

/** Decimal places used for fiat-like values (USDT and other quote currencies). */
export const MONEY_DP = 2;
/** Decimal places used for coin quantities. */
export const QTY_DP = 8;

function toNumber(value: DecimalString | null | undefined): number | null {
	if (value === null || value === undefined || value === '') return null;
	const parsed = Number(value);
	return Number.isFinite(parsed) ? parsed : null;
}

/** Formats a monetary value with 2 decimals and locale grouping. */
export function money(value: DecimalString | null | undefined, fallback = '—'): string {
	const parsed = toNumber(value);
	if (parsed === null) return fallback;

	return new Intl.NumberFormat(i18n.locale, {
		minimumFractionDigits: MONEY_DP,
		maximumFractionDigits: MONEY_DP
	}).format(parsed);
}

/**
 * Formats a coin quantity with up to 8 decimals, trailing zeros trimmed.
 *
 * `minimumFractionDigits: 0` is what stops a whole number rendering as
 * `1,00000000`, which reads as noise in a dense table.
 */
export function qty(value: DecimalString | null | undefined, fallback = '—'): string {
	const parsed = toNumber(value);
	if (parsed === null) return fallback;

	return new Intl.NumberFormat(i18n.locale, {
		minimumFractionDigits: 0,
		maximumFractionDigits: QTY_DP
	}).format(parsed);
}

/**
 * Formats a price. Prices span many orders of magnitude — BTC near 100,000 and
 * SHIB near 0.00001 — so the precision adapts rather than truncating small
 * prices to `0.00`.
 */
export function price(value: DecimalString | null | undefined, fallback = '—'): string {
	const parsed = toNumber(value);
	if (parsed === null) return fallback;

	const abs = Math.abs(parsed);
	const digits = abs >= 1000 ? 2 : abs >= 1 ? 4 : 8;

	return new Intl.NumberFormat(i18n.locale, {
		minimumFractionDigits: 2,
		maximumFractionDigits: digits
	}).format(parsed);
}

/** Formats a percentage value that is already on a 0–100 scale. */
export function percent(value: DecimalString | null | undefined, fallback = '—'): string {
	const parsed = toNumber(value);
	if (parsed === null) return fallback;

	return `${new Intl.NumberFormat(i18n.locale, {
		minimumFractionDigits: 1,
		maximumFractionDigits: 1
	}).format(parsed)}%`;
}

/** Signed money, with an explicit `+` so a gain is unambiguous at a glance. */
export function signedMoney(value: DecimalString | null | undefined, fallback = '—'): string {
	const parsed = toNumber(value);
	if (parsed === null) return fallback;
	const formatted = money(value, fallback);
	return parsed > 0 ? `+${formatted}` : formatted;
}

/**
 * Converts a UTC timestamp from the backend into the user's LOCAL time.
 *
 * All computation happens in UTC (AGENTS.md §2.3); this is the single boundary
 * where the display switches to local, and it exists so a trade's time reads the
 * way the user remembers placing it.
 */
export function dateTime(iso: string | null | undefined, fallback = '—'): string {
	if (!iso) return fallback;
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return fallback;

	return new Intl.DateTimeFormat(i18n.locale, {
		year: 'numeric',
		month: '2-digit',
		day: '2-digit',
		hour: '2-digit',
		minute: '2-digit',
		second: '2-digit'
	}).format(date);
}

/** Date-only variant for ranges and axis labels. */
export function dateOnly(iso: string | null | undefined, fallback = '—'): string {
	if (!iso) return fallback;
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return fallback;

	return new Intl.DateTimeFormat(i18n.locale, {
		year: 'numeric',
		month: '2-digit',
		day: '2-digit'
	}).format(date);
}

/** Short relative time ("3 min ago") for the price freshness indicator. */
export function relativeTime(iso: string | null | undefined): string {
	if (!iso) return '';
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return '';

	const seconds = Math.round((date.getTime() - Date.now()) / 1000);
	const formatter = new Intl.RelativeTimeFormat(i18n.locale, { numeric: 'auto' });

	const thresholds: Array<[Intl.RelativeTimeFormatUnit, number]> = [
		['second', 60],
		['minute', 60],
		['hour', 24],
		['day', 7]
	];

	let value = seconds;
	for (const [unit, step] of thresholds) {
		if (Math.abs(value) < step) return formatter.format(Math.round(value), unit);
		value /= step;
	}
	return formatter.format(Math.round(value), 'week');
}

/** Tailwind class for a P&L figure, driven by the semantic tokens in app.css. */
export function pnlClass(value: DecimalString | null | undefined): string {
	const parsed = toNumber(value);
	if (parsed === null || parsed === 0) return 'text-muted-foreground';
	return parsed > 0 ? 'text-profit' : 'text-loss';
}

/** Numeric value for sorting and charting. Never for display arithmetic. */
export function num(value: DecimalString | null | undefined): number {
	return toNumber(value) ?? 0;
}
