/** A transfer speed the way a person reads it: `1.2 MB/s`, `640 KB/s`. */
export function formatSpeed(bytesPerSecond: number, language = 'en'): string {
	if (!bytesPerSecond || bytesPerSecond < 0) return '';
	const number = (value: number, digits: number) =>
		new Intl.NumberFormat(language, {
			minimumFractionDigits: digits,
			maximumFractionDigits: digits
		}).format(value);
	const megabytes = bytesPerSecond / (1024 * 1024);
	return megabytes >= 1
		? `${number(megabytes, 1)} MB/s`
		: `${number(Math.max(1, Math.round(bytesPerSecond / 1024)), 0)} KB/s`;
}

/** How long ago something was, as `3 min` or `2 h`: enough to tell old from new in a queue. */
export function formatAge(seconds: number): string {
	const elapsed = Math.max(0, seconds);
	if (elapsed < 60) return '< 1 min';
	if (elapsed < 3600) return `${Math.floor(elapsed / 60)} min`;
	if (elapsed < 86400) return `${Math.floor(elapsed / 3600)} h`;
	return `${Math.floor(elapsed / 86400)} d`;
}
