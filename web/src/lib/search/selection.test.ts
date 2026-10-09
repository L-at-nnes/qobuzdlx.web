import { describe, expect, it } from 'vitest';
import { type Row, Selection } from './selection.svelte.ts';

const rows = (...ids: number[]): Row<number>[] =>
	ids.map((id) => ({ id, wanted: { url: `u/${id}`, label: `t${id}` } }));

const urls = (selection: Selection<number>) => selection.wanted().map((w) => w.url);

describe('Selection', () => {
	it('ticks and unticks one row at a time', () => {
		const selection = new Selection<number>();
		const list = rows(1, 2, 3);

		selection.toggle('a', list, 2);
		expect(urls(selection)).toEqual(['u/2']);

		selection.toggle('a', list, 2);
		expect(selection.size).toBe(0);
	});

	it('ticks every row between the last click and a shift-click', () => {
		const selection = new Selection<number>();
		const list = rows(1, 2, 3, 4, 5);

		selection.toggle('a', list, 2);
		selection.toggle('a', list, 4, true);

		expect(urls(selection)).toEqual(['u/2', 'u/3', 'u/4']);
	});

	it('draws a range upwards as well', () => {
		const selection = new Selection<number>();
		const list = rows(1, 2, 3, 4, 5);

		selection.toggle('a', list, 4);
		selection.toggle('a', list, 2, true);

		expect(selection.size).toBe(3);
		expect(selection.has(3)).toBe(true);
	});

	it('unticks a range when the row clicked is being unticked', () => {
		const selection = new Selection<number>();
		const list = rows(1, 2, 3, 4);
		selection.setAll(list, true);

		selection.toggle('a', list, 1);
		selection.toggle('a', list, 3, true);

		expect(urls(selection)).toEqual(['u/4']);
	});

	it('takes a shift-click for a plain one when there is no click before it', () => {
		const selection = new Selection<number>();

		selection.toggle('a', rows(1, 2, 3), 3, true);

		expect(urls(selection)).toEqual(['u/3']);
	});

	it('never draws a range from one list into another', () => {
		const selection = new Selection<number>();

		selection.toggle('album', rows(1, 2, 3), 1);
		selection.toggle('playlist', rows(4, 5, 6), 6, true);

		expect(urls(selection)).toEqual(['u/1', 'u/6']);
	});

	it('keeps what to queue after the list it came from is gone', () => {
		const selection = new Selection<number>();
		selection.toggle('a', rows(7), 7);

		expect(selection.wanted()).toEqual([{ url: 'u/7', label: 't7' }]);
	});

	it('forgets the last click when cleared', () => {
		const selection = new Selection<number>();
		const list = rows(1, 2, 3);
		selection.toggle('a', list, 1);
		selection.clear();

		selection.toggle('a', list, 3, true);

		expect(urls(selection)).toEqual(['u/3']);
	});
});
