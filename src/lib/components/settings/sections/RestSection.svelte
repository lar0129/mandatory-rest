<script lang="ts">
  import { settings } from '$lib/stores/settings';
  import { previewRestReminder, setSetting } from '$lib/ipc';
  import SettingsToggle from '$lib/components/settings/SettingsToggle.svelte';
  import * as m from '$paraglide/messages.js';

  let message = $state($settings.rest_reminder_message);
  let previewing = $state(false);

  $effect(() => {
    message = $settings.rest_reminder_message;
  });

  async function toggle(key: string, current: boolean) {
    const updated = await setSetting(key, current ? 'false' : 'true');
    settings.set(updated);
  }

  async function saveNumber(key: string, raw: number, min: number, max: number) {
    const value = Math.min(max, Math.max(min, Math.round(raw || min)));
    const updated = await setSetting(key, String(value));
    settings.set(updated);
  }

  async function saveMessage() {
    const value = message.trim() || m.rest_default_message();
    message = value;
    if (value === $settings.rest_reminder_message) return;
    const updated = await setSetting('rest_reminder_message', value);
    settings.set(updated);
  }

  async function preview() {
    if (previewing) return;
    await saveMessage();
    previewing = true;
    try {
      await previewRestReminder();
    } finally {
      previewing = false;
    }
  }
</script>

<div class="section">
  <div class="group-heading">{m.rest_group_schedule()}</div>

  <SettingsToggle
    label={m.rest_toggle_enabled()}
    description={m.rest_toggle_enabled_desc()}
    checked={$settings.rest_reminder_enabled}
    onclick={() => toggle('rest_reminder_enabled', $settings.rest_reminder_enabled)}
  />

  <div class="fields" class:disabled={!$settings.rest_reminder_enabled}>
    <label class="field-row">
      <span class="field-meta">
        <span class="label">{m.rest_interval()}</span>
        <span class="desc">{m.rest_interval_desc()}</span>
      </span>
      <span class="number-control">
        <input
          type="number"
          min="1"
          max="240"
          value={Math.round($settings.rest_reminder_interval_secs / 60)}
          onchange={(event) =>
            saveNumber(
              'rest_reminder_interval_secs',
              (event.currentTarget as HTMLInputElement).valueAsNumber * 60,
              60,
              240 * 60
            )}
        />
        <span>{m.rest_minutes()}</span>
      </span>
    </label>

    <label class="field-row">
      <span class="field-meta">
        <span class="label">{m.rest_duration()}</span>
        <span class="desc">{m.rest_duration_desc()}</span>
      </span>
      <span class="number-control">
        <input
          type="number"
          min="5"
          max="900"
          value={$settings.rest_reminder_duration_secs}
          onchange={(event) =>
            saveNumber(
              'rest_reminder_duration_secs',
              (event.currentTarget as HTMLInputElement).valueAsNumber,
              5,
              900
            )}
        />
        <span>{m.rest_seconds()}</span>
      </span>
    </label>
  </div>

  <div class="group-heading">{m.rest_group_pause_protection()}</div>

  <div class="fields" class:disabled={!$settings.rest_reminder_enabled}>
    <label class="field-row">
      <span class="field-meta">
        <span class="label">{m.rest_pause_warning_interval()}</span>
        <span class="desc">{m.rest_pause_warning_interval_desc()}</span>
      </span>
      <span class="number-control">
        <input
          type="number"
          min="1"
          max="240"
          value={Math.round($settings.rest_reminder_pause_warning_secs / 60)}
          onchange={(event) =>
            saveNumber(
              'rest_reminder_pause_warning_secs',
              (event.currentTarget as HTMLInputElement).valueAsNumber * 60,
              60,
              240 * 60
            )}
        />
        <span>{m.rest_minutes()}</span>
      </span>
    </label>
  </div>

  <div class="group-heading">{m.rest_group_overlay()}</div>

  <div class="message-row" class:disabled={!$settings.rest_reminder_enabled}>
    <label for="rest-message" class="label">{m.rest_message()}</label>
    <span class="desc">{m.rest_message_desc()}</span>
    <textarea
      id="rest-message"
      maxlength="240"
      bind:value={message}
      onblur={saveMessage}
      onkeydown={(event) => {
        if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
          event.preventDefault();
          void saveMessage();
          (event.currentTarget as HTMLTextAreaElement).blur();
        }
      }}></textarea>
  </div>

  <SettingsToggle
    label={m.rest_toggle_skip()}
    description={m.rest_toggle_skip_desc()}
    checked={$settings.rest_reminder_allow_skip}
    onclick={() => toggle('rest_reminder_allow_skip', $settings.rest_reminder_allow_skip)}
  />

  <div class="preview-row">
    <p>{m.rest_pomodoro_desc()}</p>
    <button onclick={preview} disabled={previewing}>{m.rest_preview()}</button>
  </div>
</div>

<style>
  .section {
    display: flex;
    flex-direction: column;
  }

  .group-heading {
    padding: 16px 20px 6px;
    color: var(--color-foreground-darker, var(--color-foreground));
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    opacity: 0.6;
  }

  .fields,
  .message-row {
    transition: opacity 0.15s;
  }

  .disabled {
    opacity: 0.4;
    pointer-events: none;
  }

  .field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 12px 20px;
    border-bottom: 1px solid var(--color-separator);
  }

  .field-meta {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
  }

  .label {
    color: var(--color-foreground);
    font-size: 0.85rem;
    letter-spacing: 0.02em;
  }

  .desc {
    color: var(--color-foreground-darker, var(--color-foreground));
    font-size: 0.72rem;
    letter-spacing: 0.02em;
    opacity: 0.7;
  }

  .number-control {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 7px;
    color: var(--color-foreground-darker, var(--color-foreground));
    font-size: 0.72rem;
  }

  input,
  textarea {
    border: 1px solid color-mix(in oklch, var(--color-foreground) 20%, transparent);
    border-radius: 4px;
    outline: none;
    background: var(--color-hover);
    color: var(--color-foreground);
    font: inherit;
  }

  input {
    width: 72px;
    padding: 5px 7px;
    font-family: monospace;
    text-align: right;
  }

  .message-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 20px;
    border-bottom: 1px solid var(--color-separator);
  }

  textarea {
    min-height: 84px;
    margin-top: 6px;
    padding: 9px 10px;
    resize: vertical;
    line-height: 1.45;
  }

  input:focus,
  textarea:focus {
    border-color: color-mix(in oklch, var(--color-foreground) 42%, transparent);
  }

  .preview-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 14px 20px;
  }

  .preview-row p {
    margin: 0;
    color: var(--color-foreground-darker, var(--color-foreground));
    font-size: 0.72rem;
    line-height: 1.45;
  }

  .preview-row button {
    flex-shrink: 0;
    padding: 6px 13px;
    border: 1px solid color-mix(in oklch, var(--color-foreground) 20%, transparent);
    border-radius: 4px;
    background: var(--color-hover);
    color: var(--color-foreground);
    font-size: 0.75rem;
    cursor: pointer;
  }

  .preview-row button:hover:not(:disabled) {
    background: color-mix(in oklch, var(--color-foreground) 17%, transparent);
  }

  .preview-row button:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
