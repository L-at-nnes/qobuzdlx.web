import { describe, expect, it } from 'vitest';
import { en } from './en';
import { fr } from './fr';

type Message = string | { one: string; other: string };

function forms(message: Message): string[] {
	return typeof message === 'string' ? [message] : [message.one, message.other];
}

function placeholders(text: string): string[] {
	return [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
}

const english = en as Record<string, Message>;
const french = fr as Record<string, Message>;

describe('the catalogues', () => {
	it('have the same keys', () => {
		expect(Object.keys(french).sort()).toEqual(Object.keys(english).sort());
	});

	it('use the same placeholders in each language, form by form', () => {
		for (const key of Object.keys(english)) {
			const wanted = new Set(forms(english[key]).flatMap(placeholders));
			for (const text of forms(french[key])) {
				// A plural form may leave out the count it says in words ("un", "une")
				const used = placeholders(text).filter((name) => name !== 'count');
				for (const name of used) {
					expect(wanted, `${key}: {${name}} in French`).toContain(name);
				}
			}
			for (const text of forms(french[key])) {
				for (const name of wanted) {
					if (name === 'count') continue;
					expect(text, `${key}: French lacks {${name}}`).toContain(`{${name}}`);
				}
			}
		}
	});

	it('give a plural message the same shape in both languages', () => {
		for (const key of Object.keys(english)) {
			expect(typeof french[key], key).toBe(typeof english[key]);
		}
	});

	it('hold no em dash, and no exclamation mark in English', () => {
		for (const [name, catalogue] of [
			['en', english],
			['fr', french]
		] as const) {
			for (const [key, message] of Object.entries(catalogue)) {
				for (const text of forms(message)) {
					expect(text, `${name}.${key}`).not.toContain('—');
					if (name === 'en') expect(text, `en.${key}`).not.toContain('!');
				}
			}
		}
	});

	it('hold no empty text', () => {
		for (const catalogue of [english, french]) {
			for (const [key, message] of Object.entries(catalogue)) {
				for (const text of forms(message)) expect(text.trim(), key).not.toBe('');
			}
		}
	});
});
