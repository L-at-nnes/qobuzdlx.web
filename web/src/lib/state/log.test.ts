import { describe, expect, it } from 'vitest';
import type { LogLine } from '#lib/types.ts';
import { mergeLog } from './log.ts';

const line = (seq: number): LogLine => ({ seq, level: 'normal', event: 'done' }) as LogLine;
const seqs = (lines: LogLine[]) => lines.map((entry) => entry.seq);

describe('mergeLog', () => {
	it('adds to the history the lines told while it was being read', () => {
		const merged = mergeLog([line(0), line(1), line(2)], [line(2), line(3), line(4)], 100);

		expect(seqs(merged.lines)).toEqual([0, 1, 2, 3, 4]);
		expect(merged.lastSeq).toBe(4);
	});

	it('starts again from the history of a server that restarted', () => {
		// The page had shown lines up to 900; the new run of the server numbers from zero
		const merged = mergeLog([line(0), line(1)], [line(2)], 100);

		expect(seqs(merged.lines)).toEqual([0, 1, 2]);
		expect(merged.lastSeq).toBe(2);
	});

	it('knows an empty history, so that the first line of a new run is taken', () => {
		expect(mergeLog([], [], 100)).toEqual({ lines: [], lastSeq: -1 });
		expect(mergeLog([], [line(0)], 100).lastSeq).toBe(0);
	});

	it('keeps only the latest lines', () => {
		const merged = mergeLog([line(0), line(1), line(2)], [line(3)], 2);

		expect(seqs(merged.lines)).toEqual([2, 3]);
	});
});
