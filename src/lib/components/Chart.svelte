<script lang="ts">
	// ECharts wrapper.
	//
	// Canvas renderer, not SVG: the timeline chart plots one point per trade and
	// canvas stays smooth into the tens of thousands where SVG would create that
	// many DOM nodes. Canvas is also what makes `getDataURL()` produce the PNG the
	// HTML report embeds.
	//
	// Styling comes from a registered ECharts theme (see charts/theme.ts), never
	// from merging style keys into each chart's option. Spreading a caller's
	// option over hand-built defaults silently dropped whole components — an
	// earlier version lost every tooltip that way, because the caller's `tooltip`
	// key replaced the styled one wholesale instead of merging into it.
	import * as echarts from 'echarts';
	import { onMount, untrack } from 'svelte';

	import { registerChart, unregisterChart } from '$lib/charts/registry';
	import { currentThemeName, ensureThemesRegistered, exportBackground } from '$lib/charts/theme';
	import { app } from '$lib/stores/app.svelte';

	let {
		id,
		title,
		option,
		height = 340
	}: {
		id: string;
		title: string;
		option: echarts.EChartsOption;
		height?: number;
	} = $props();

	let container = $state<HTMLDivElement | null>(null);
	let chart: echarts.ECharts | null = null;
	let mountedTheme = '';
	let observer: ResizeObserver | null = null;

	function create() {
		if (!container) return;

		ensureThemesRegistered();
		mountedTheme = currentThemeName();
		chart = echarts.init(container, mountedTheme, { renderer: 'canvas' });
		render();
	}

	function destroy() {
		chart?.dispose();
		chart = null;
	}

	function render() {
		if (!chart) return;
		// `notMerge: true` — series counts change when the user switches pairs,
		// and a merge would leave the previous chart's series behind.
		chart.setOption(option, { notMerge: true });
	}

	onMount(() => {
		create();

		registerChart(id, title, () =>
			chart
				? chart.getDataURL({
						type: 'png',
						pixelRatio: 2,
						// The report may be printed on white paper; a transparent
						// background would leave the labels invisible.
						backgroundColor: exportBackground()
					})
				: null
		);

		if (container) {
			observer = new ResizeObserver(() => chart?.resize());
			observer.observe(container);
		}

		return () => {
			observer?.disconnect();
			observer = null;
			unregisterChart(id);
			destroy();
		};
	});

	// Re-render on data change, and rebuild entirely when the theme flips —
	// a registered theme is bound at `init` time and cannot be swapped in place.
	$effect(() => {
		void option;
		void app.settings.theme;

		untrack(() => {
			if (!chart) return;
			if (currentThemeName() !== mountedTheme) {
				destroy();
				create();
			} else {
				render();
			}
		});
	});
</script>

<div bind:this={container} style="height: {height}px" class="w-full"></div>
