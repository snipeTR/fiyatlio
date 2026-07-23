// vite.config.ts
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// Set by `tauri dev` when developing against a physical device; undefined otherwise.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	// Tailwind v4 is configured entirely in CSS (`src/app.css`) — there is no
	// tailwind.config.js and no PostCSS pipeline.
	plugins: [tailwindcss(), sveltekit()],

	// Tauri owns the terminal output; don't let Vite wipe it.
	clearScreen: false,

	server: {
		// Must match `build.devUrl` in src-tauri/tauri.conf.json.
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
		watch: {
			// Rust rebuilds are Cargo's job; watching them here causes reload loops.
			ignored: ['**/src-tauri/**']
		}
	},

	// Produce readable stack traces in debug builds, small bundles in release.
	build: {
		target: 'chrome105',
		minify: process.env.TAURI_ENV_DEBUG ? false : 'esbuild',
		sourcemap: !!process.env.TAURI_ENV_DEBUG
	}
});
