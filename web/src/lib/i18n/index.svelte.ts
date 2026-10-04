import type { Language, LogLine } from '#lib/types.ts';
import { en, type Key } from './en';
import { fr } from './fr';

export type { Key };

/** A message, with one form per plural category when its wording changes with a count. */
export type Message = string | { one: string; other: string };

const catalogues: Record<Language, Record<Key, Message>> = { en, fr };

export const LANGUAGES: { code: Language; name: string }[] = [
	{ code: 'en', name: 'English' },
	{ code: 'fr', name: 'Français' }
];

const STORAGE_KEY = 'qbdlx.language';

/** The language the browser asks for, as far as the interface speaks it. */
export function detectLanguage(preferred: readonly string[]): Language {
	for (const tag of preferred) {
		const base = tag.toLowerCase().split('-')[0];
		if (base === 'fr' || base === 'en') return base;
	}
	return 'en';
}

function remembered(): Language | null {
	try {
		const value = localStorage.getItem(STORAGE_KEY);
		return value === 'fr' || value === 'en' ? value : null;
	} catch {
		return null;
	}
}

class Locale {
	current = $state<Language>('en');

	/** Chooses the language at start-up: what was chosen before, else the browser's. */
	start() {
		this.current =
			remembered() ??
			detectLanguage(typeof navigator === 'undefined' ? [] : (navigator.languages ?? []));
		this.apply();
	}

	/** Sets the language, and remembers it for the screens shown before the server can be asked. */
	set(language: Language) {
		this.current = language;
		try {
			localStorage.setItem(STORAGE_KEY, language);
		} catch {
			// Private browsing: the choice lasts until the page is closed
		}
		this.apply();
	}

	private apply() {
		if (typeof document !== 'undefined') document.documentElement.lang = this.current;
	}
}

export const locale = new Locale();

function pick(message: Message, count: number | undefined): string {
	if (typeof message === 'string') return message;
	const form = new Intl.PluralRules(locale.current).select(count ?? 0);
	return form === 'one' ? message.one : message.other;
}

/** The message for a key, in the current language, with `{name}` replaced by the parameters. */
export function t(key: Key, params: Record<string, string | number> = {}): string {
	const message = catalogues[locale.current][key] ?? catalogues.en[key];
	const text = pick(message, typeof params.count === 'number' ? params.count : undefined);
	return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
		name in params ? String(params[name]) : whole
	);
}

/** A number the way the current language writes it. */
export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
	return new Intl.NumberFormat(locale.current, options).format(value);
}

/** The words of a line of the log, in the current language. */
export function describeLog(line: LogLine): string {
	const key = `log.${line.event}` as Key;
	if (!(key in en)) return line.event;
	const fields: Record<string, string | number> = {};
	for (const [name, value] of Object.entries(line)) {
		if (typeof value === 'string' || typeof value === 'number') fields[name] = value;
	}
	return t(key, fields);
}
