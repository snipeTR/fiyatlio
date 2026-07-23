// src/lib/i18n/index.ts
//
// Minimal rune-based i18n.
//
// Hand-rolled rather than pulling in a library: the app has exactly two locales,
// no pluralisation rules beyond simple interpolation, and no lazy loading (both
// dictionaries together are a few kilobytes). A library would add an async
// initialisation dance that fights Svelte 5's synchronous rune model for no
// benefit here.
//
// Key safety: `en` is typed as `Record<keyof typeof tr, string>`, so adding a key
// to Turkish without adding it to English fails the build. `TranslationKey` is
// derived from the same source, so `t('typo.key')` is also a compile error.

import { en } from './en';
import { tr } from './tr';
import type { Language } from '$lib/types';

export type TranslationKey = keyof typeof tr;

const DICTIONARIES: Record<Language, Record<TranslationKey, string>> = {
	tr,
	en
};

/** Locale tag used for `Intl` number and date formatting. */
export const LOCALE_TAGS: Record<Language, string> = {
	tr: 'tr-TR',
	en: 'en-US'
};

class I18nState {
	current = $state<Language>('tr');

	/**
	 * Looks up `key` and substitutes `{name}` placeholders.
	 *
	 * A missing key returns the key itself rather than throwing or rendering
	 * blank — a visible `dashboard.foo` in the UI is the fastest possible signal
	 * that a string is missing, and it never breaks the page.
	 */
	t = (key: TranslationKey, params?: Record<string, string | number>): string => {
		const template = DICTIONARIES[this.current][key] ?? key;
		if (!params) return template;

		return template.replace(/\{(\w+)\}/g, (match, name: string) => {
			const value = params[name];
			return value === undefined ? match : String(value);
		});
	};

	get locale(): string {
		return LOCALE_TAGS[this.current];
	}

	set = (language: Language) => {
		this.current = language;
		if (typeof document !== 'undefined') {
			document.documentElement.lang = language;
		}
	};
}

export const i18n = new I18nState();

/** Convenience binding so components can `import { t }`. */
export const t = i18n.t;
