/** Flyout sections: the 5h hero, today's rows, notes and the footer, built once and updated in place. */
import { left, money, tokens } from './format.ts';
import { bar, el, modelRows, show, toggle } from './parts.ts';
import type { Snapshot } from './types.ts';

const BLOCK_MS = 5 * 3_600_000;
/** Hours and minutes in the user's own style ("5:00 PM" or "17:00"), like the taskbar clock and the badge. */
const clock = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' });
const EMPTY = { tokens: 0, cost: 0, unpriced: false, models: [] };

/** True while a 5h window is running at `now`. */
function running(s: Snapshot | null, now: number): s is Snapshot & { windowEnd: number } {
  return !!s && s.windowEnd !== null && s.windowEnd > now;
}

/** The 5h card: cost, tokens, reset time and a bar of the time used; says so when idle or when no logs exist. */
function hero(): { node: HTMLElement; set: (s: Snapshot | null, now: number) => void } {
  const head = el('div', 'head');
  const label = head.appendChild(el('span', 'label'));
  const resets = head.appendChild(el('span', 'muted'));
  const [big, sub, remain] = [el('div', 'big'), el('div', 'sub muted'), el('div', 'remain muted')];
  const time = bar('time');
  const rows = modelRows();
  const card = el('header', 'hero');
  card.append(head, big, sub, time.node, remain);
  const node = el('div');
  node.append(card, rows.node);
  return {
    node,
    set: (s, now) => {
      const found = s?.rootFound ?? true;
      const active = running(s, now);
      label.textContent = found ? '5-hour window' : 'No logs found';
      big.textContent = found ? money(s?.window.cost ?? 0, s?.window.unpriced) : 'Nothing to read yet';
      big.classList.toggle('idle', !active);
      sub.textContent = !s
        ? 'Reading logs…'
        : !found
          ? `Juice reads ${s.root}. Run Claude Code once, or set CLAUDE_CONFIG_DIR.`
          : active
            ? `${tokens(s.window.tokens)} tokens`
            : 'No active window. It starts with your next request.';
      resets.textContent = active ? `resets ${clock.format(s.windowEnd)}` : '';
      show(time.node, active);
      show(remain, active);
      if (active) {
        time.set(1 - (s.windowEnd - now) / BLOCK_MS);
        remain.textContent = left(s.windowEnd - now);
      }
      rows.set(active ? s.window : EMPTY);
    },
  };
}

/** Today's total and model rows. */
function todaySection(): { node: HTMLElement; set: (s: Snapshot | null) => void } {
  const node = el('section', 'today');
  const head = node.appendChild(el('div', 'head'));
  head.append(el('span', 'label', 'Today'));
  const total = head.appendChild(el('span', 'muted'));
  const rows = modelRows();
  node.append(rows.node);
  return {
    node,
    set: (s) => {
      show(node, !!s && s.rootFound);
      if (!s) return;
      total.textContent = `${money(s.today.cost, s.today.unpriced)} · ${tokens(s.today.tokens)} tokens`;
      rows.set(s.today);
    },
  };
}

/** Warnings: unreadable log lines, and models without a published price. */
function notes(): { node: HTMLElement; set: (s: Snapshot | null, now: number) => void } {
  const warn = el('p', 'note warn');
  const unpriced = el('p', 'note muted', '+ Some models have no published price: their tokens count, their cost is not guessed.');
  const node = el('div');
  node.append(warn, unpriced);
  return {
    node,
    set: (s, now) => {
      const n = s?.unreadable ?? 0;
      show(warn, n > 0);
      warn.textContent = `${n} log line${n === 1 ? '' : 's'} could not be read and ${n === 1 ? 'is' : 'are'} not counted.`;
      show(unpriced, !!s && (s.today.unpriced || (running(s, now) && s.window.unpriced)));
    },
  };
}

export interface Actions {
  quit: () => void;
  /** Each turns its setting on or off and resolves to the state actually set. */
  autostart: (on: boolean) => Promise<boolean>;
  badge: (on: boolean) => Promise<boolean>;
}

/** Settings switches, Quit, and the fine print. */
function footer(act: Actions): { node: HTMLElement; setSwitches: (autostart: boolean, badge: boolean) => void } {
  const startup = toggle('Start with Windows', act.autostart);
  const taskbar = toggle('Show on taskbar', act.badge);
  const switches = el('div', 'switches');
  switches.append(startup.node, taskbar.node);
  const quit = el('button', 'quit', 'Quit');
  quit.addEventListener('click', act.quit);
  const row = el('footer');
  row.append(switches, quit);
  const node = el('div');
  node.append(row, el('p', 'fine muted', 'Estimates at API list prices · unofficial, not affiliated with Anthropic'));
  return {
    node,
    setSwitches: (autostart, badge) => {
      startup.input.checked = autostart;
      taskbar.input.checked = badge;
    },
  };
}

export interface View {
  render: (s: Snapshot | null, now: number) => void;
  setSwitches: (autostart: boolean, badge: boolean) => void;
  /** Plays the open motion. */
  enter: () => void;
}

/** Builds the flyout inside `root`. */
export function mount(root: HTMLElement, act: Actions): View {
  const [top, today, note, foot] = [hero(), todaySection(), notes(), footer(act)];
  root.append(top.node, today.node, note.node, foot.node);
  return {
    render: (s, now) => {
      top.set(s, now);
      today.set(s);
      note.set(s, now);
    },
    setSwitches: foot.setSwitches,
    enter: () => {
      root.classList.remove('enter');
      void root.offsetWidth;
      root.classList.add('enter');
    },
  };
}
