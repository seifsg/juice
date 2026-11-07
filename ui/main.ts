/** Flyout entry: wires the core's snapshots and commands to the view, and sizes the window to its content. */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { report } from './parts.ts';
import type { Snapshot } from './types.ts';
import { mount } from './view.ts';

const root = document.querySelector<HTMLElement>('#app');
if (!root) throw new Error('juice: #app missing');

/** Counts switch flips, so a system read that started before a flip cannot undo it. */
let flips = 0;
const view = mount(root, {
  quit: () => void invoke('quit'),
  autostart: (on) => (flips++, invoke<boolean>('set_autostart', { on })),
  badge: (on) => (flips++, invoke<boolean>('set_badge', { on })),
});
let last: Snapshot | null = null;
const show = (s: Snapshot | null): void => {
  last = s;
  view.render(s, Date.now());
};

/** Reads both switches from the system, so a change made elsewhere (Task Manager) shows on every open.
 * A flip during the read wins: its own result sets the switch. */
const syncSwitches = async (): Promise<void> => {
  const before = flips;
  const [autostart, badge] = await Promise.all([invoke<boolean>('autostart'), invoke<boolean>('badge')]);
  if (flips === before) view.setSwitches(autostart, badge);
};

// The window hugs its content; the core places it over the tray icon.
new ResizeObserver(() => {
  invoke('fit', { height: Math.ceil(root.getBoundingClientRect().height) }).catch((e: unknown) => report('fit', e));
}).observe(root);

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') invoke('close').catch((err: unknown) => report('close', err));
});

try {
  await listen<Snapshot>('snapshot', (e) => show(e.payload));
  // An event may already have arrived: keep it rather than the older first-paint copy.
  const first = await invoke<Snapshot | null>('snapshot');
  if (!last) show(first);
  await syncSwitches();
} catch (e) {
  report('start', e);
  show(last);
}
view.enter();

// Countdown and time bar move on their own, only while the flyout is visible.
setInterval(() => document.hidden || view.render(last, Date.now()), 30_000);
// Every open focuses the window: fresh numbers, the open motion, and switches re-read from the system.
window.addEventListener('focus', () => {
  view.render(last, Date.now());
  view.enter();
  syncSwitches().catch((e: unknown) => report('switches', e));
});
