import { writable } from 'svelte/store';
import type { RestReminderState } from '$lib/types';

const initial: RestReminderState = {
  active: false,
  message: '',
  remaining_secs: 0,
  allow_skip: false,
  reason: '',
  enabled: true,
  timer_remaining_secs: 60 * 60,
  timer_total_secs: 60 * 60,
  is_paused: false,
  pause_reason: '',
  next_pause_warning_secs: 0,
};

export const restReminderState = writable<RestReminderState>(initial);
