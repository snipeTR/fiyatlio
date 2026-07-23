// src/lib/charts/theme.ts
//
// ECharts themes that match the application's own palette.
//
// Registering real themes rather than merging style keys into every chart
// option: ECharts' own defaults are tuned for a white page — axis labels and
// legend text default to near-black (#333/#333), which on this app's dark
// background is invisible, and on a *light* card the legend text was
// inheriting a light colour and disappearing the other way. A theme sets those
// defaults once, for every component (legend, axes, dataZoom, tooltip,
// toolbox), instead of leaving each chart to remember them.

import * as echarts from 'echarts';

export const DARK_THEME = 'fiyatlio-dark';
export const LIGHT_THEME = 'fiyatlio-light';

/** Series palette. Distinguishable in both themes and colour-blind safe enough. */
const SERIES_PALETTE = [
	'#60a5fa',
	'#a78bfa',
	'#f472b6',
	'#fbbf24',
	'#34d399',
	'#22d3ee',
	'#fb923c',
	'#c084fc',
	'#4ade80',
	'#f87171'
];

interface Tokens {
	text: string;
	muted: string;
	axisLine: string;
	split: string;
	tooltipBg: string;
	tooltipBorder: string;
}

const DARK: Tokens = {
	text: '#e2e8f0',
	muted: '#94a3b8',
	axisLine: 'rgba(255,255,255,0.22)',
	split: 'rgba(255,255,255,0.08)',
	tooltipBg: 'rgba(30,41,59,0.97)',
	tooltipBorder: 'rgba(255,255,255,0.14)'
};

const LIGHT: Tokens = {
	text: '#0f172a',
	muted: '#475569',
	axisLine: 'rgba(15,23,42,0.25)',
	split: 'rgba(15,23,42,0.08)',
	tooltipBg: 'rgba(255,255,255,0.98)',
	tooltipBorder: 'rgba(15,23,42,0.14)'
};

function buildTheme(tokens: Tokens) {
	const axis = {
		axisLine: { show: true, lineStyle: { color: tokens.axisLine } },
		axisTick: { show: true, lineStyle: { color: tokens.axisLine } },
		axisLabel: { show: true, color: tokens.muted },
		splitLine: { show: true, lineStyle: { color: tokens.split } },
		splitArea: { show: false }
	};

	return {
		color: SERIES_PALETTE,
		backgroundColor: 'transparent',
		textStyle: { color: tokens.text },

		title: {
			textStyle: { color: tokens.text },
			subtextStyle: { color: tokens.muted }
		},

		// The legend is where the unreadable-labels problem showed up: it lists
		// coin names, and its text colour is the one ECharts hard-defaults.
		legend: {
			textStyle: { color: tokens.text },
			inactiveColor: tokens.muted,
			pageTextStyle: { color: tokens.muted },
			pageIconColor: tokens.muted,
			pageIconInactiveColor: tokens.split
		},

		tooltip: {
			backgroundColor: tokens.tooltipBg,
			borderColor: tokens.tooltipBorder,
			borderWidth: 1,
			textStyle: { color: tokens.text },
			extraCssText: 'box-shadow: 0 8px 24px rgba(0,0,0,0.28); border-radius: 8px;',
			axisPointer: {
				lineStyle: { color: tokens.axisLine },
				crossStyle: { color: tokens.axisLine },
				label: { color: tokens.text, backgroundColor: tokens.tooltipBg }
			}
		},

		categoryAxis: axis,
		valueAxis: axis,
		timeAxis: axis,
		logAxis: axis,

		dataZoom: {
			textStyle: { color: tokens.muted },
			borderColor: tokens.split,
			fillerColor: 'rgba(96,165,250,0.16)',
			handleStyle: { color: tokens.muted },
			moveHandleStyle: { color: tokens.muted },
			dataBackground: {
				lineStyle: { color: tokens.split },
				areaStyle: { color: tokens.split }
			},
			selectedDataBackground: {
				lineStyle: { color: tokens.muted },
				areaStyle: { color: 'rgba(96,165,250,0.24)' }
			}
		},

		// Pie labels sit over the card, not over the slice, so they need the
		// foreground colour too.
		pie: {
			label: { color: tokens.text },
			labelLine: { lineStyle: { color: tokens.axisLine } }
		}
	};
}

let registered = false;

/** Registers both themes once per page load. */
export function ensureThemesRegistered(): void {
	if (registered) return;
	echarts.registerTheme(DARK_THEME, buildTheme(DARK));
	echarts.registerTheme(LIGHT_THEME, buildTheme(LIGHT));
	registered = true;
}

/** Resolved theme name for the document's current appearance. */
export function currentThemeName(): string {
	const dark =
		typeof document !== 'undefined' && document.documentElement.classList.contains('dark');
	return dark ? DARK_THEME : LIGHT_THEME;
}

/** Background to bake into exported PNGs, which have no page behind them. */
export function exportBackground(): string {
	return currentThemeName() === DARK_THEME ? '#0f172a' : '#ffffff';
}
