import { ApiError, api } from '#lib/api.ts';
import { t } from '#lib/i18n/index.svelte.ts';
import { toast } from './toast.svelte.ts';

export interface Wanted {
	url: string;
	label: string;
	sub_label?: string;
	thumbnail?: string;
}

/** Puts things in the download queue and says what became of them. Returns whether any were added. */
export async function enqueue(items: Wanted[]): Promise<boolean> {
	if (items.length === 0) return false;
	try {
		const result = await api.enqueue(items);
		const queued = result.added + result.requeued;
		if (queued > 0) {
			toast.show(t('queue.added', { count: queued }));
		} else if (result.duplicates > 0) {
			toast.show(t('queue.duplicates', { count: result.duplicates }));
		} else if (result.invalid > 0) {
			toast.error(t('queue.invalid'));
		}
		return queued > 0;
	} catch (error) {
		if (!(error instanceof ApiError && (error.isLocked || error.isSessionExpired))) {
			toast.error(t('queue.failed'));
		}
		return false;
	}
}
