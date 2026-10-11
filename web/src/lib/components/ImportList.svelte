<script lang="ts">
	import { ApiError, api } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { toast } from '#lib/state/toast.svelte.ts';
	import Button from './Button.svelte';

	/** What the server reads: the same bound as its own. */
	const MAX_BYTES = 16 * 1024 * 1024;

	let input: HTMLInputElement;
	let busy = $state(false);

	async function chosen() {
		const file = input.files?.[0];
		// The same file chosen twice has to fire again
		input.value = '';
		if (!file) return;
		if (file.size > MAX_BYTES) {
			toast.error(t('import.tooBig'));
			return;
		}

		busy = true;
		try {
			const counts = await api.importList(await file.text());
			toast.show(
				t('import.done', {
					added: counts.added + counts.requeued,
					duplicates: counts.duplicates,
					ignored: counts.invalid
				})
			);
		} catch (error) {
			if (error instanceof ApiError && error.code === 'nothing_to_add') {
				toast.error(t('import.empty'));
			} else if (!(error instanceof ApiError && (error.isLocked || error.isSessionExpired))) {
				toast.error(t('queue.failed'));
			}
		} finally {
			busy = false;
		}
	}
</script>

<input
	bind:this={input}
	class="visually-hidden"
	type="file"
	accept=".txt,text/plain"
	tabindex="-1"
	aria-hidden="true"
	onchange={chosen}
/>
<Button variant="quiet" icon="upload" loading={busy} title={t('import.hint')} onclick={() => input.click()}>
	{t('import.button')}
</Button>
