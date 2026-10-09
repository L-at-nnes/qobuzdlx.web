<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { Snippet } from 'svelte';
	import Cover from './Cover.svelte';
	import Icon from './Icon.svelte';

	interface Props {
		title: string;
		meta?: string;
		thumbnail?: string | null;
		round?: boolean;
		explicit?: boolean;
		/** Present when the row opens onto more (the tracks of an album). */
		open?: boolean;
		ontoggle?: () => void;
		openLabel?: string;
		closeLabel?: string;
		/** Present when the row can be selected. */
		selected?: boolean;
		/** `range` is true on a shift-click: every row since the last click is picked with this one. */
		onselect?: (range: boolean) => void;
		badges?: Snippet;
		actions?: Snippet;
		children?: Snippet;
	}

	let {
		title,
		meta,
		thumbnail,
		round = false,
		explicit = false,
		open = false,
		ontoggle,
		openLabel,
		closeLabel,
		selected,
		onselect,
		badges,
		actions,
		children
	}: Props = $props();

	const uid = $props.id();
	const expandable = $derived(ontoggle !== undefined);

	/** Ctrl-click or shift-click on a row picks it, as in a file explorer; a plain click opens it. */
	function openOrPick(event: MouseEvent) {
		if (onselect && (event.ctrlKey || event.metaKey || event.shiftKey)) {
			onselect(event.shiftKey);
		} else {
			ontoggle?.();
		}
	}
</script>

<li class="row" class:open class:selected={selected}>
	<div class="line">
		{#if onselect}
			<input
				class="check"
				type="checkbox"
				checked={selected}
				onclick={(event) => onselect(event.shiftKey)}
				aria-label={t('row.select', { title })}
			/>
		{/if}

		{#if expandable}
			<button
				class="main"
				type="button"
				aria-expanded={open}
				aria-controls="panel-{uid}"
				title={open ? closeLabel : openLabel}
				onclick={openOrPick}
			>
				<Cover src={thumbnail} {round} />
				<span class="text">
					<span class="work-title">{title}</span>
					{#if meta}<span class="meta">{meta}</span>{/if}
				</span>
				<span class="chevron" class:turned={open}><Icon name="chevron-right" size={18} /></span>
			</button>
		{:else}
			<div class="main static">
				<Cover src={thumbnail} {round} />
				<span class="text">
					<span class="work-title">{title}</span>
					{#if meta}<span class="meta">{meta}</span>{/if}
				</span>
			</div>
		{/if}

		<div class="side">
			{#if explicit}<span class="explicit" title={t('row.explicit')}>E</span>{/if}
			{@render badges?.()}
			{@render actions?.()}
		</div>
	</div>

	{#if open && children}
		<div class="panel" id="panel-{uid}">{@render children()}</div>
	{/if}
</li>

<style>
	.row {
		border-bottom: 1px solid var(--line-soft);
	}

	.row.open {
		background: var(--surface);
	}

	.row.selected {
		background: color-mix(in oklch, var(--bg), var(--accent) 7%);
	}

	.line {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-2);
	}

	.check {
		flex: none;
		width: 18px;
		height: 18px;
		margin: 0;
		accent-color: var(--accent);
		cursor: pointer;
	}

	.main {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: 0;
		border: 0;
		background: none;
		text-align: left;
		color: inherit;
	}

	button.main {
		cursor: pointer;
		user-select: none;
		border-radius: var(--radius-sm);
	}

	.text {
		display: grid;
		gap: 2px;
		min-width: 0;
	}

	.chevron {
		margin-left: auto;
		display: inline-grid;
		color: var(--text-faint);
		transition: transform var(--duration-fast) var(--ease-out);
	}

	.chevron.turned {
		transform: rotate(90deg);
	}

	.side {
		flex: none;
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.explicit {
		display: inline-grid;
		place-items: center;
		width: 18px;
		height: 18px;
		border-radius: 3px;
		background: var(--raised);
		color: var(--text-muted);
		font-size: 0.6875rem;
		font-weight: 700;
	}

	.panel {
		padding: 0 var(--space-2) var(--space-3) calc(var(--space-2) + 18px + var(--space-3));
	}

	@media (max-width: 44rem) {
		.line {
			align-items: flex-start;
			flex-wrap: wrap;
		}

		.check {
			margin-top: 14px;
		}

		.main {
			flex-basis: calc(100% - 18px - var(--space-3));
		}

		.side {
			margin-left: calc(18px + var(--space-3) + 48px + var(--space-3));
		}

		.chevron {
			display: none;
		}

		.panel {
			padding-left: var(--space-2);
		}
	}
</style>
