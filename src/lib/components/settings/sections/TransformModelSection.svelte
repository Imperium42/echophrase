<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Transform Model Section
   *
   * Post-processing settings for transcription output through a local LLM.
   * Allows users to trade speed for better text quality (grammar/spelling normalization).
   */

  import { SettingsSection, SettingToggle, HintTooltip } from '../primitives';
  import { getSettingsContext } from '../context';
  import { DEFAULT_TRANSFORM_RULES } from '$lib/types/settings';
  import { getModelById, formatSize } from '$lib/types/models';
  import { transformModelStore } from '$lib/stores/transformModelStore';
  import { t } from '$lib/locales';

  const DEFAULT_TRANSFORM_MODEL_ID = 't5-mini-grammar';

  interface Props {
    /** Open the transformation-model picker dialog (mounted by SettingsModal). */
    onChangeModel?: () => void;
  }

  let { onChangeModel }: Props = $props();

  const { settings, updateSetting } = getSettingsContext();

  // Local state for rules textarea
  let rulesValue = $state('');

  // Sync rules from settings
  $effect(() => {
    rulesValue = $settings?.transform_rules || DEFAULT_TRANSFORM_RULES;
  });

  // Derived values
  const isEnabled = $derived($settings?.transform_enabled ?? false);
  // Validate the persisted id against the transformation category, mirroring
  // transformModelStore. A stale or mis-migrated `transform_model` can hold a
  // TRANSCRIPTION model id, and without this guard the section rendered that
  // voice model (name, size, language badge) as the transform model - and keyed
  // `isModelReady` off the wrong file.
  const selectedModel = $derived.by(() => {
    const persisted = $settings?.transform_model;
    if (!persisted) return DEFAULT_TRANSFORM_MODEL_ID;
    const def = getModelById(persisted);
    return def?.category === 'transformation' ? persisted : DEFAULT_TRANSFORM_MODEL_ID;
  });

  // Resolve display metadata from the canonical TS registry.
  const modelDisplayInfo = $derived.by(() => {
    const def = getModelById(selectedModel);
    if (!def) {
      return { name: selectedModel, meta: 'ONNX', metaWarning: null, languages: null };
    }
    const sizeLabel = def.sizeBytes > 0 ? formatSize(def.sizeBytes) : '';
    return {
      name: def.name,
      meta: `${sizeLabel} · ONNX`,
      metaWarning: null,
      languages: def.languages ?? null,
    };
  });

  // All current transform models are fine-tuned (T5-Mini Grammar) - no
  // instruction-tuned LLM tier ships yet, so custom rules stay hidden.
  const isFineTuned = $derived(true);

  // Drives the "Setup" badge + toggle gate. No bundled tier currently;
  // readiness is purely whether the active model is on disk.
  const isModelReady = $derived(!!$transformModelStore.downloadedModels[selectedModel]);

  function handleToggle(value: boolean) {
    // Block enabling when the selected DLC model isn't downloaded yet -
    // open the picker instead so the user can install it before turning on.
    if (value && !isModelReady) {
      onChangeModel?.();
      return;
    }
    updateSetting('transform_enabled', value);
  }

  function handleRulesBlur() {
    const trimmed = rulesValue.trim();
    if (trimmed !== ($settings?.transform_rules || DEFAULT_TRANSFORM_RULES)) {
      updateSetting('transform_rules', trimmed || DEFAULT_TRANSFORM_RULES);
    }
  }

  function handleChangeModel() {
    onChangeModel?.();
  }
</script>

<SettingsSection id="transform-model" title={t('transformModel.title')} icon="transform">
  {#snippet titleExtra()}
    <HintTooltip hint={t('transformModel.helpTooltip')} />
  {/snippet}
  <div class="section-subtitle">
    {t('transformModel.subtitle')}
    <span class="localizing-hint" title={t('transformModel.localizingTooltip')}
      >{t('transformModel.localizingHint')}</span
    >
  </div>

  <!-- Enable Toggle. Uses `hint` (neutral "?") not `warning` (amber ⚠) -
	     the tooltip is a usage tip about paragraph length, not a danger. -->
  <SettingToggle
    label={t('transformModel.enableToggle')}
    hint={t('transformModel.enableWarning')}
    checked={isEnabled}
    onchange={handleToggle}
  />

  <!-- Model Selection Row. When the active transform_model isn't ready,
	     the row shows a "Not installed" placeholder and the Change button
	     is replaced by an amber Setup button that opens the picker. -->
  <div class="model-row">
    <div class="model-info">
      {#if isModelReady}
        <span class="model-name">
          {modelDisplayInfo.name}
          {#if modelDisplayInfo.languages}
            <span class="lang-badge" title={t('transformModel.languageBadgeTooltip')}
              >{modelDisplayInfo.languages}</span
            >
          {/if}
          {#if isFineTuned}
            <HintTooltip hint={t('transformModel.finetuneTooltip')} />
          {/if}
        </span>
        <span class="model-meta"
          >{modelDisplayInfo.meta}{#if modelDisplayInfo.metaWarning}&nbsp;<span class="warning-text"
              >({modelDisplayInfo.metaWarning})</span
            >{/if}</span
        >
      {:else}
        <span class="model-name placeholder">{t('transformModel.noneSelected')}</span>
      {/if}
    </div>
    {#if isModelReady}
      <button class="change-btn" onclick={handleChangeModel}>
        {t('modelSettings.change')}
      </button>
    {:else}
      <button
        class="setup-btn"
        onclick={handleChangeModel}
        title={t('transformModel.setupRequiredTooltip')}
      >
        {t('transformModel.setupBadge')}
      </button>
    {/if}
  </div>

  <!-- TODO: Rules textarea is hidden while we only support the tiny bundled model (t5-efficient-tiny-grammar).
	     Re-enable when we add flexible transform model selection (e.g. larger LLMs that accept custom prompts). -->
  <!-- Rules Textarea (hidden - fine-tuned model ignores custom rules)
	<div
		class="rules-section"
		class:disabled={!isEnabled}
		class:fine-tuned={isFineTuned && isEnabled}
		title={isFineTuned && isEnabled ? 'Fine-tuned model: Rules ignored' : undefined}
	>
		<div class="rules-header">
			<div class="rules-label">Rules</div>
			<button
				class="reset-btn"
				title="Reset to default"
				onclick={() => {
					rulesValue = DEFAULT_TRANSFORM_RULES;
					updateSetting('transform_rules', DEFAULT_TRANSFORM_RULES);
				}}
				disabled={!isEnabled || isFineTuned}
			>
				<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
				</svg>
			</button>
		</div>
		<textarea
			class="rules-textarea"
			bind:value={rulesValue}
			onblur={handleRulesBlur}
			disabled={!isEnabled || isFineTuned}
			placeholder={DEFAULT_TRANSFORM_RULES}
			rows="3"
		></textarea>
	</div>
	-->
</SettingsSection>

<style>

  .section-subtitle {
    font-size: 0.6875rem;
    color: #52525b;
    margin-bottom: 0.75rem;
  }

  .warning-text {
    color: #ca8a04;
  }

  .localizing-hint {
    color: #a1a1aa;
    text-decoration: underline;
    text-decoration-style: dotted;
    text-underline-offset: 2px;
    cursor: help;
  }

  .model-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0;
  }

  .model-info {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    flex: 1;
    min-width: 0;
  }

  .model-name {
    font-size: 0.8125rem;
    color: #d4d4d8;
  }

  .model-meta {
    font-size: 0.6875rem;
    color: #52525b;
  }

  .change-btn {
    padding: 0.375rem 0.625rem;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 450;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .change-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.04);
    color: #a1a1aa;
  }

  .change-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .setup-btn {
    padding: 0.375rem 0.75rem;
    background: rgba(234, 179, 8, 0.15);
    border: 1px solid rgba(234, 179, 8, 0.3);
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 600;
    color: #eab308;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease,
      border-color 0.15s ease;
  }

  .setup-btn:hover {
    background: rgba(234, 179, 8, 0.25);
    border-color: rgba(234, 179, 8, 0.5);
    color: #facc15;
  }

  .model-name.placeholder {
    color: #71717a;
    font-style: italic;
  }

  .lang-badge {
    font-size: 0.5625rem;
    font-weight: 600;
    color: #60a5fa;
    background: rgba(96, 165, 250, 0.15);
    padding: 0.0625rem 0.25rem;
    border-radius: 0.1875rem;
    margin-left: 0.375rem;
    vertical-align: middle;
    cursor: help;
  }
</style>
