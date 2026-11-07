/** Flyout DOM: built once, updated in place so bars ease between snapshots. Log data only ever lands in textContent. */
import { left, model, money, tokens } from './format.ts';
import type { ModelRow, Period, Snapshot } from './types.ts';

const BLOCK_MS = 5 * 3_600_000;
const clock = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

/** Element with a class and plain text. */
function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls = '', text = ''): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text) e.textContent = text;
  return e;
}

/** Thin track with an accent fill; `set` takes a 0..1 share. */
function bar(cls: string): { node: HTMLElement; set: (share: number) => void } {
  const node = el('div', `bar ${cls}`);
  const fill = node.appendChild(el('div', 'fill'));
  return { node, set: (share) => (fill.style.width = `${Math.min(1, Math.max(0, share)) * 100}%`) };
}

/** One model's line: name, tokens, cost, and a bar sized against the period's top cost. */
function modelRow(): { node: HTMLElement; set: (m: ModelRow, top: number) => void } {
  const node = el('div', 'row');
  const [name, tok, cost] = [el('span', 'name'), el('span', 'num muted'), el('span', 'num')];
  const b = bar('thin');
  node.append(name, tok, cost, b.node);
  return {
    node,
    set: (m, top) => {
      name.textContent = model(m.model);
      tok.textContent = tokens(m.tokens.input + m.tokens.output + m.tokens.cacheWrite + m.tokens.cacheRead);
      const priced = m.cost > 0 || !m.unpriced;
      cost.textContent = priced ? money(m.cost, m.unpriced) : 'no price';
      cost.classList.toggle('muted', !priced);
      node.title = `${m.model}\nin ${tokens(m.tokens.input)} · out ${tokens(m.tokens.output)} · cache write ${tokens(m.tokens.cacheWrite)} · cache read ${tokens(m.tokens.cacheRead)}`;
      b.set(top > 0 ? m.cost / top : 0);
    },
  };
}

/** A period's model rows, keyed by model id so existing rows move and resize instead of being rebuilt. */
function modelRows(): { node: HTMLElement; set: (p: Period) => void } {
  const node = el('div', 'rows');
  const rows = new Map<string, ReturnType<typeof modelRow>>();
  return {
    node,
    set: (p) => {
      const top = p.models[0]?.cost ?? 0;
      const live = new Set(p.models.map((m) => m.model));
      for (const [id, r] of rows) if (!live.has(id)) (r.node.remove(), rows.delete(id));
      for (const m of p.models) {
        const r = rows.get(m.model) ?? modelRow();
        rows.set(m.model, r);
        r.set(m, top);
        node.append(r.node);
      }
    },
  };
}

/** A switch that asks `set` for a new state and shows the state it resolves to (reverts on failure). */
function toggle(label: string, set: (on: boolean) => Promise<boolean>): { node: HTMLElement; input: HTMLInputElement } {
  const input = el('input');
  input.type = 'checkbox';
  input.setAttribute('role', 'switch');
  const node = el('label', 'switch');
  node.append(input, el('span', 'track'), el('span', '', label));
  input.addEventListener('change', () => {
    const want = input.checked;
    set(want).then(
      (on) => (input.checked = on),
      (e: unknown) => ((input.checked = !want), console.error(`juice: ${label}`, e)),
    );
  });
  return { node, input };
}

export interface Actions {
  quit: () => void;
  /** Each turns its setting on or off and resolves to the state actually set. */
  autostart: (on: boolean) => Promise<boolean>;
  badge: (on: boolean) => Promise<boolean>;
}

export interface View {
  render: (s: Snapshot | null, now: number) => void;
  setSwitches: (autostart: boolean, badge: boolean) => void;
  /** Plays the open motion. */
  enter: () => void;
}

/** Builds the flyout inside `root`. */
export function mount(root: HTMLElement, act: Actions): View {
  const head = el('div', 'head');
  const label = head.appendChild(el('span', 'label', '5-hour window'));
  const resets = head.appendChild(el('span', 'muted'));
  const big = el('div', 'big');
  const sub = el('div', 'sub muted');
  const time = bar('time');
  const remain = el('div', 'remain muted');
  const windowRows = modelRows();
  const today = el('section', 'today');
  const todayHead = today.appendChild(el('div', 'head'));
  todayHead.append(el('span', 'label', 'Today'));
  const todayTotal = todayHead.appendChild(el('span', 'muted'));
  const todayRows = modelRows();
  today.append(todayRows.node);
  const warn = el('p', 'note warn');
  const unpriced = el('p', 'note muted', '+ Some models have no published price: their tokens count, their cost is not guessed.');

  const startup = toggle('Start with Windows', act.autostart);
  const taskbar = toggle('Show on taskbar', act.badge);
  const switches = el('div', 'switches');
  switches.append(startup.node, taskbar.node);
  const quit = el('button', 'quit', 'Quit');
  quit.addEventListener('click', act.quit);
  const footer = el('footer');
  footer.append(switches, quit);
  const fine = el('p', 'fine muted', 'Estimates at API list prices · unofficial, not affiliated with Anthropic');

  const hero = el('header', 'hero');
  hero.append(head, big, sub, time.node, remain);
  root.append(hero, windowRows.node, today, warn, unpriced, footer, fine);

  const show = (node: HTMLElement, on: boolean) => node.toggleAttribute('hidden', !on);

  return {
    render(s, now) {
      const found = s?.rootFound ?? true;
      const active = !!s && s.windowEnd !== null && s.windowEnd > now;
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
      resets.textContent = active && s?.windowEnd ? `resets ${clock.format(s.windowEnd)}` : '';
      show(time.node, active);
      show(remain, active);
      if (active && s?.windowEnd) {
        time.set(1 - (s.windowEnd - now) / BLOCK_MS);
        remain.textContent = left(s.windowEnd - now);
      }
      windowRows.set(active && s ? s.window : { tokens: 0, cost: 0, unpriced: false, models: [] });
      show(today, !!s && found);
      if (s) {
        todayTotal.textContent = `${money(s.today.cost, s.today.unpriced)} · ${tokens(s.today.tokens)} tokens`;
        todayRows.set(s.today);
      }
      show(warn, !!s && s.unreadable > 0);
      warn.textContent = s ? `${s.unreadable} log line${s.unreadable === 1 ? '' : 's'} could not be read and ${s.unreadable === 1 ? 'is' : 'are'} not counted.` : '';
      show(unpriced, !!s && (s.today.unpriced || (active && s.window.unpriced)));
    },
    setSwitches(autostart, badge) {
      startup.input.checked = autostart;
      taskbar.input.checked = badge;
    },
    enter() {
      root.classList.remove('enter');
      void root.offsetWidth;
      root.classList.add('enter');
    },
  };
}
