import { describe, expect, it } from 'vitest';
import { formatAge, formatSpeed } from './format';

describe('speeds', () => {
	it('are written in megabytes from one megabyte a second', () => {
		expect(formatSpeed(1.2 * 1024 * 1024)).toBe('1.2 MB/s');
		expect(formatSpeed(25 * 1024 * 1024)).toBe('25.0 MB/s');
	});

	it('are written in kilobytes below that', () => {
		expect(formatSpeed(640 * 1024)).toBe('640 KB/s');
		expect(formatSpeed(10)).toBe('1 KB/s');
	});

	it('use the decimal mark of the language', () => {
		expect(formatSpeed(1.5 * 1024 * 1024, 'fr')).toBe('1,5 MB/s');
	});

	it('are empty when nothing is moving', () => {
		expect(formatSpeed(0)).toBe('');
		expect(formatSpeed(-3)).toBe('');
	});
});

describe('ages', () => {
	it('tell old from new', () => {
		expect(formatAge(10)).toBe('< 1 min');
		expect(formatAge(180)).toBe('3 min');
		expect(formatAge(7300)).toBe('2 h');
		expect(formatAge(200000)).toBe('2 d');
	});
});
