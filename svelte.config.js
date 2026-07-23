// svelte.config.js
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		// Tauri serves a static bundle from disk — there is no Node server.
		// Every route is known at build time, so we fully prerender and skip the
		// SPA fallback (a fallback named index.html would clobber the prerendered
		// root page). `trailingSlash: 'always'` in +layout.ts makes each route emit
		// `<route>/index.html`, which resolves correctly off the filesystem.
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			precompress: false,
			strict: true
		}),
		alias: {
			$lib: './src/lib'
		}
	}
};

export default config;
