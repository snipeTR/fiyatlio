<script lang="ts">
	import { page } from '$app/state';
	import { t } from '$lib/i18n/index.svelte';
	import { app } from '$lib/stores/app.svelte';
	import Icon, { type IconName } from '$lib/components/ui/Icon.svelte';

	// `trailingSlash: 'always'` in +layout.ts means every route resolves with a
	// trailing slash, so hrefs carry one and the active check compares against
	// the normalised pathname.
	const LINKS: Array<{ href: string; labelKey: Parameters<typeof t>[0]; icon: IconName }> = [
		{ href: '/', labelKey: 'nav.dashboard', icon: 'dashboard' },
		{ href: '/portfolio/', labelKey: 'nav.portfolio', icon: 'wallet' },
		{ href: '/trades/', labelKey: 'nav.trades', icon: 'list' },
		{ href: '/charts/', labelKey: 'nav.charts', icon: 'chart' },
		{ href: '/reports/', labelKey: 'nav.reports', icon: 'report' },
		{ href: '/settings/', labelKey: 'nav.settings', icon: 'settings' }
	];

	function isActive(href: string): boolean {
		const current = page.url.pathname;
		return href === '/' ? current === '/' : current.startsWith(href);
	}
</script>

<nav class="flex h-full w-56 shrink-0 flex-col border-r border-border bg-card/40">
	<div class="px-5 py-5">
		<div class="text-base font-semibold tracking-tight">{t('app.name')}</div>
		<div class="mt-0.5 text-[11px] text-muted-foreground">{t('app.tagline')}</div>
	</div>

	<ul class="flex flex-1 flex-col gap-0.5 px-3">
		{#each LINKS as link (link.href)}
			<li>
				<a
					href={link.href}
					class="flex items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors
						{isActive(link.href)
						? 'bg-accent font-medium text-accent-foreground'
						: 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
					aria-current={isActive(link.href) ? 'page' : undefined}
				>
					<Icon name={link.icon} size={16} />
					{t(link.labelKey)}
				</a>
			</li>
		{/each}
	</ul>

	<div class="px-5 py-4 text-[11px] text-muted-foreground">
		{t('common.version')} {app.version || '—'}
	</div>
</nav>
