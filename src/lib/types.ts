// src/lib/types.ts
//
// TypeScript mirror of the Rust models in `src-tauri/src/models.rs`.
//
// TWO RULES that this file exists to enforce:
//   1. Every monetary / quantity value arrives as a STRING, not a number. The
//      backend serialises `rust_decimal::Decimal` via `serde::str` precisely so
//      an 8-decimal crypto quantity does not round-trip through an IEEE-754
//      double. Parse with `Number(...)` for DISPLAY only — never to compute a
//      figure the user will act on. All arithmetic belongs in Rust.
//   2. Enum string values match serde's `rename_all`, so they are camelCase
//      except `Side`, which is UPPERCASE.

/** A decimal value carried as an exact string. */
export type DecimalString = string;

export type Side = 'BUY' | 'SELL';
export type PnlMethod = 'fifo' | 'averageCost';
export type OversellPolicy = 'zeroCostBasis' | 'ignoreExcess';
export type Language = 'tr' | 'en';
export type ThemeMode = 'dark' | 'light' | 'system';
export type FeeKind = 'none' | 'base' | 'quote' | 'thirdAsset';

export type ExportKind = 'trades' | 'realizedEvents';
export type ExportFormat = 'csv' | 'json';

export interface Trade {
	id: number;
	timestampUtc: string;
	pair: string;
	baseAsset: string;
	quoteAsset: string;
	side: Side;
	price: DecimalString;
	qty: DecimalString;
	quoteAmount: DecimalString;
	feeAmount: DecimalString;
	feeAsset: string;
	sourceFile: string;
	fileIndex: number;
	rowIndex: number;
}

export interface Lot {
	qtyRemaining: DecimalString;
	/** Authoritative remaining cost — see the note on `Lot` in models.rs. */
	costRemaining: DecimalString;
	/** Display only. Never use for accounting. */
	unitCostQuote: DecimalString;
	acquiredAt: string;
	tradeId: number;
}

export interface MatchedLot {
	buyTradeId: number | null;
	acquiredAt: string | null;
	qty: DecimalString;
	unitCostQuote: DecimalString;
	cost: DecimalString;
	/** True when the slice is not backed by a discrete tracked lot. */
	synthetic: boolean;
}

export interface RealizedEvent {
	sellTradeId: number;
	timestamp: string;
	pair: string;
	baseAsset: string;
	quoteAsset: string;
	qty: DecimalString;
	proceeds: DecimalString;
	costBasis: DecimalString;
	pnl: DecimalString;
	matchedLots: MatchedLot[];
	oversell: boolean;
	oversellQty: DecimalString;
}

export interface Position {
	pair: string;
	baseAsset: string;
	quoteAsset: string;
	qty: DecimalString;
	totalCost: DecimalString;
	avgCost: DecimalString;
	realizedPnl: DecimalString;
	feesByAsset: Record<string, DecimalString>;
	buyCount: number;
	sellCount: number;
	hasOversell: boolean;
	openLots: Lot[];
}

export interface PositionValuation {
	position: Position;
	currentPrice: DecimalString | null;
	marketValueQuote: DecimalString | null;
	unrealizedPnl: DecimalString | null;
	marketValueUsdt: DecimalString | null;
	realizedPnlUsdt: DecimalString | null;
	unrealizedPnlUsdt: DecimalString | null;
	convertibleToUsdt: boolean;
}

export interface RowError {
	file: string;
	fileIndex: number;
	rowIndex: number;
	/** i18n key, e.g. `error.row.badSide`. */
	reasonKey: string;
	detail: string;
	raw: string;
}

export interface FileImportStats {
	file: string;
	fileIndex: number;
	totalRows: number;
	validRows: number;
	invalidRows: number;
	duplicateRows: number;
}

export interface ImportSummary {
	files: FileImportStats[];
	totalRows: number;
	validRows: number;
	invalidRows: number;
	duplicateRows: number;
	firstTradeAt: string | null;
	lastTradeAt: string | null;
	pairs: string[];
	errors: RowError[];
}

export interface PriceSnapshot {
	prices: Record<string, DecimalString>;
	fetchedAt: string;
	/** True when the snapshot came from cache rather than a live fetch. */
	stale: boolean;
	failedSymbols: string[];
}

export interface DashboardSummary {
	totalRealizedPnlUsdt: DecimalString;
	totalUnrealizedPnlUsdt: DecimalString;
	totalPortfolioValueUsdt: DecimalString;
	winRate: DecimalString;
	winningEvents: number;
	totalSellEvents: number;
	totalTrades: number;
	totalFeesByAsset: Record<string, DecimalString>;
	totalFeesUsdtApprox: DecimalString;
	feesConversionPartial: boolean;
	excludedPairs: string[];
	hasOversell: boolean;
	/** How many disposals exceeded tracked inventory. */
	oversellEventCount: number;
	/** Realized P&L those disposals contributed, in USDT. */
	oversellPnlUsdt: DecimalString;
	/** Base quantity sold with no tracked acquisition, per asset. */
	oversellQtyByAsset: Record<string, DecimalString>;
	priceSnapshot: PriceSnapshot | null;
}

export interface AnalysisResult {
	method: PnlMethod;
	summary: DashboardSummary;
	positions: PositionValuation[];
	realizedEvents: RealizedEvent[];
	import: ImportSummary;
}

export interface Settings {
	pnlMethod: PnlMethod;
	oversellPolicy: OversellPolicy;
	deduplicate: boolean;
	autoRefresh: boolean;
	autoRefreshSecs: number;
	language: Language;
	theme: ThemeMode;
	lastFiles: string[];
}

/** Minimum auto-refresh interval, mirrored from `models::MIN_AUTO_REFRESH_SECS`. */
export const MIN_AUTO_REFRESH_SECS = 30;

/** Chart image handed to the report generator. */
export interface ChartImage {
	title: string;
	/** `data:image/png;base64,…` from ECharts' `getDataURL()`. */
	dataUri: string;
}

export interface ReportOptions {
	language: Language;
	charts: ChartImage[];
}

/**
 * The shape `AppError` serialises into. `messageKey` is an i18n key; `detail` is
 * untranslated technical context and is only shown in the expandable part of an
 * error toast.
 */
export interface AppErrorPayload {
	kind: string;
	messageKey: string;
	detail: string;
}

export function isAppError(value: unknown): value is AppErrorPayload {
	return (
		typeof value === 'object' &&
		value !== null &&
		'kind' in value &&
		'messageKey' in value &&
		'detail' in value
	);
}
