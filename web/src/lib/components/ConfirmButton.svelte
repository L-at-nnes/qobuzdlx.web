<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import Button from './Button.svelte';
	import type { IconName } from './Icon.svelte';

	let {
		label,
		question,
		confirmLabel,
		icon,
		onconfirm,
		disabled = false
	}: {
		label: string;
		question: string;
		confirmLabel: string;
		icon?: IconName;
		onconfirm: () => void;
		disabled?: boolean;
	} = $props();

	let asking = $state(false);
</script>

{#if asking}
	<span class="ask" role="group" aria-label={question}>
		<span class="question">{question}</span>
		<Button
			size="sm"
			variant="danger"
			onclick={() => {
				asking = false;
				onconfirm();
			}}
		>
			{confirmLabel}
		</Button>
		<Button size="sm" variant="ghost" onclick={() => (asking = false)}>{t('common.cancel')}</Button>
	</span>
{:else}
	<Button size="sm" variant="quiet" {icon} {disabled} onclick={() => (asking = true)}>{label}</Button>
{/if}

<style>
	.ask {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		flex-wrap: wrap;
	}

	.question {
		font-size: var(--text-sm);
		font-weight: 600;
	}
</style>
