// src/lib/charts/registry.ts
//
// Lets the Reports page collect PNG snapshots of charts that live on the Charts
// page.
//
// A plain module-level Map rather than reactive state: nothing renders from this,
// it is only read at the moment a report is generated. Each `Chart` component
// registers a getter on mount and removes it on destroy, so a chart that is not
// currently mounted simply contributes nothing instead of producing a stale image.

export type ChartSnapshot = () => string | null;

const registry = new Map<string, { title: string; snapshot: ChartSnapshot }>();

export function registerChart(id: string, title: string, snapshot: ChartSnapshot): void {
	registry.set(id, { title, snapshot });
}

export function unregisterChart(id: string): void {
	registry.delete(id);
}

/** Current titles, for showing the user what a report would include. */
export function availableCharts(): string[] {
	return [...registry.values()].map((entry) => entry.title);
}

/**
 * Captures every mounted chart as a data URI.
 *
 * Charts that fail to render are skipped rather than aborting the report — a
 * report missing one graphic is far more useful than no report.
 */
export function captureCharts(): Array<{ title: string; dataUri: string }> {
	const captured: Array<{ title: string; dataUri: string }> = [];

	for (const { title, snapshot } of registry.values()) {
		try {
			const dataUri = snapshot();
			if (dataUri && dataUri.startsWith('data:image/')) {
				captured.push({ title, dataUri });
			}
		} catch (error) {
			console.warn(`chart "${title}" could not be captured`, error);
		}
	}

	return captured;
}
