// src/routes/+layout.ts
//
// Tauri loads a prerendered static bundle from disk and every Tauri API is
// browser-only, so server-side rendering must stay off for the whole app.

export const prerender = true;
export const ssr = false;
export const trailingSlash = 'always';
