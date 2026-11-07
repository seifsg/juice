/** Flyout building blocks. Log data only ever lands in textContent, never in HTML. */
import { model, money, tokens } from './format.ts';
import type { ModelRow, Period } from './types.ts';

/** Element with a class and plain text. */
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls = '', text = ''): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text) e.textContent = text;
  return e;
}

/** Shows or hides a node. */
export function show(node: HTMLElement, on: boolean): void {
  node.toggleAttribute('hidden', !on);
}

/** Thin track with an accent fill; `set` takes a 0..1 share. */
export function bar(cls: string): { node: HTMLElement; set: (share: number) => void } {
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
      const t = m.tokens;
      name.textContent = model(m.model);
      tok.textContent = tokens(t.input + t.output + t.cacheWrite + t.cacheRead);
      const priced = m.cost > 0 || !m.unpriced;
      cost.textContent = priced ? money(m.cost, m.unpriced) : 'no price';
      cost.classList.toggle('muted', !priced);
      node.title = `${m.model}\nin ${tokens(t.input)} · out ${tokens(t.output)} · cache write ${tokens(t.cacheWrite)} · cache read ${tokens(t.cacheRead)}`;
      b.set(top > 0 ? m.cost / top : 0);
    },
  };
}

/** A period's model rows, keyed by model id: rows stay in the page and only move when the order changes,
 * so their bars ease to new widths. */
export function modelRows(): { node: HTMLElement; set: (p: Period) => void } {
  const node = el('div', 'rows');
  const rows = new Map<string, ReturnType<typeof modelRow>>();
  return {
    node,
    set: (p) => {
      const top = p.models[0]?.cost ?? 0;
      const live = new Set(p.models.map((m) => m.model));
      for (const [id, r] of rows) {
        if (live.has(id)) continue;
        r.node.remove();
        rows.delete(id);
      }
      p.models.forEach((m, i) => {
        const r = rows.get(m.model) ?? modelRow();
        rows.set(m.model, r);
        r.set(m, top);
        if (node.children[i] !== r.node) node.insertBefore(r.node, node.children[i] ?? null);
      });
    },
  };
}

/** A switch that asks `set` for a new state and shows the state it resolves to (reverts on failure). */
export function toggle(label: string, set: (on: boolean) => Promise<boolean>): { node: HTMLElement; input: HTMLInputElement } {
  const input = el('input');
  input.type = 'checkbox';
  input.setAttribute('role', 'switch');
  const node = el('label', 'switch');
  node.append(input, el('span', 'track'), el('span', '', label));
  input.addEventListener('change', () => {
    const want = input.checked;
    set(want).then(
      (on) => (input.checked = on),
      (e: unknown) => {
        input.checked = !want;
        console.error(`juice: ${label}`, e);
      },
    );
  });
  return { node, input };
}
