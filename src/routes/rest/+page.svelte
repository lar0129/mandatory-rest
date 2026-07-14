<script lang="ts">
  import '../../app.css';
  import { onMount } from 'svelte';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { dismissRestReminder, getRestReminderState, getSettings } from '$lib/ipc';
  import { setLocale } from '$lib/locale.svelte.js';
  import type { RestReminderState } from '$lib/types';
  import * as m from '$paraglide/messages.js';

  let reminder = $state<RestReminderState>({
    active: false,
    message: '',
    remaining_secs: 0,
    allow_skip: false,
    reason: '',
    enabled: true,
    timer_remaining_secs: 0,
    timer_total_secs: 0,
    is_paused: false,
    pause_reason: '',
    next_pause_warning_secs: 0,
  });
  let remaining = $state(0);
  let dismissing = $state(false);

  function formatTime(totalSeconds: number): string {
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  async function dismiss() {
    if (!reminder.allow_skip || dismissing) return;
    dismissing = true;
    await dismissRestReminder();
  }

  onMount(() => {
    const win = getCurrentWebviewWindow();
    let interval: ReturnType<typeof setInterval> | undefined;
    let deadline = 0;

    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && reminder.allow_skip) {
        event.preventDefault();
        void dismiss();
      }
    };
    document.addEventListener('keydown', handleKeydown);

    (async () => {
      try {
        const [settings, state] = await Promise.all([getSettings(), getRestReminderState()]);
        setLocale(settings.language);
        reminder = state;
        remaining = state.remaining_secs;
        if (!state.active) {
          await win.close();
          return;
        }

        deadline = Date.now() + state.remaining_secs * 1000;
        await win.show();
        if (win.label.endsWith('0')) await win.setFocus();

        interval = setInterval(() => {
          remaining = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
        }, 250);
      } catch {
        await win.close();
      }
    })();

    return () => {
      if (interval) clearInterval(interval);
      document.removeEventListener('keydown', handleKeydown);
    };
  });
</script>

<svelte:head>
  <title>{m.rest_overlay_title()}</title>
</svelte:head>

<div class="overlay" role="dialog" aria-modal="true" aria-label={m.rest_overlay_title()}>
  <main>
    <p class="message">{reminder.message || m.rest_default_message()}</p>
    <div class="countdown" aria-live="polite">{formatTime(remaining)}</div>
    {#if reminder.allow_skip}
      <button onclick={dismiss} disabled={dismissing}>{m.rest_overlay_skip()}</button>
      <span class="hint">{m.rest_overlay_skip_hint()}</span>
    {/if}
  </main>
</div>

<style>
  :global(html),
  :global(body) {
    width: 100%;
    height: 100%;
    margin: 0;
    overflow: hidden;
    background: #000;
    color: #fff;
  }

  .overlay {
    width: 100vw;
    height: 100vh;
    display: grid;
    place-items: center;
    background: #000;
    user-select: none;
  }

  main {
    width: min(900px, calc(100vw - 64px));
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 28px;
    text-align: center;
  }

  .message {
    margin: 0;
    max-width: 100%;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: clamp(2rem, 5vw, 5rem);
    font-weight: 500;
    line-height: 1.2;
    letter-spacing: -0.02em;
  }

  .countdown {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: clamp(3rem, 8vw, 8rem);
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.72);
  }

  button {
    min-width: 140px;
    padding: 12px 24px;
    border: 1px solid rgba(255, 255, 255, 0.28);
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    cursor: pointer;
    transition:
      background 0.15s ease,
      border-color 0.15s ease;
  }

  button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.16);
    border-color: rgba(255, 255, 255, 0.48);
  }

  button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .hint {
    margin-top: -18px;
    color: rgba(255, 255, 255, 0.38);
    font-size: 0.78rem;
  }
</style>
