import type { LogLine } from '#lib/types.ts';

/**
 * The log to show once its history has been read from the server: the history, then the lines the
 * live view told while it was being read that the history does not hold yet.
 *
 * The numbers of the lines only mean something within one run of the server: after a restart they
 * start again from zero, so what was shown before the history was read says nothing about what is
 * new. The history decides.
 */
export function mergeLog(
	history: LogLine[],
	told: LogLine[],
	max: number
): { lines: LogLine[]; lastSeq: number } {
	const last = history.at(-1)?.seq ?? -1;
	const late = told.filter((line) => line.seq > last);
	const lines = [...history, ...late].slice(-max);
	return { lines, lastSeq: lines.at(-1)?.seq ?? -1 };
}
