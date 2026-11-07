/** Flyout entry: wires the core's snapshots and commands to the view, and sizes the window to its content. */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Snapshot } from './types.ts';
import { mount } from './view.ts';

const root = document.querySelector<HTMLElement>('#app');
if (!root) throw new Error('juice: #app missing');

const view = mount(root, {
  quit: () => void invoke('quit'),
  autostart: (on) => invoke<boolean>('set_autostart', { on }),
  badge: (on) => invoke<boolean>('set_badge', { on }),
});
let last: Snapshot | null = null;
const show = (s: Snapshot | null): void => {
  last = s;
  view.render(s, Date.now());
};

// The window hugs its content; the core places it over the tray icon.
new ResizeObserver(() => {
  invoke('fit', { height: Math.ceil(root.getBoundingClientRect().height) }).catch((e: unknown) => console.error('juice: fit', e));
}).observe(root);

await listen<Snapshot>('snapshot', (e) => show(e.payload));
show(await invoke<Snapshot | null>('snapshot'));
view.setSwitches(await invoke<boolean>('autostart'), await invoke<boolean>('badge'));
view.enter();

// Countdown and time bar move on their own, only while the flyout is visible.
setInterval(() => document.hidden || view.render(last, Date.now()), 30_000);
document.addEventListener('visibilitychange', () => {
  if (document.hidden) return;
  view.render(last, Date.now());
  view.enter();
});
