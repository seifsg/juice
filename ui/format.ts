/** Pure formatters for the flyout; the tray tooltip uses the same rules in Rust. */

/** "$3.42", "$142": cents only while they matter. "+" marks a lower bound (some usage had no price). */
export function money(usd: number, unpriced = false): string {
  return `$${usd < 99.995 ? usd.toFixed(2) : usd.toFixed(0)}${unpriced ? '+' : ''}`;
}

/** "950", "9.9k", "340k", "1.2M", "1.2B". Units switch where rounding would print "1000k" or "10.0k". */
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  const [div, unit] = n < 999_500 ? [1e3, 'k'] : n < 999_500_000 ? [1e6, 'M'] : [1e9, 'B'];
  const v = n / div;
  return `${v < 9.95 ? v.toFixed(1) : v.toFixed(0)}${unit}`;
}

/** "claude-opus-5-5" -> "Opus 5.5", "claude-3-5-sonnet-20241022" -> "Sonnet 3.5". Other ids stay as they are. */
export function model(id: string): string {
  if (!id.startsWith('claude-')) return id;
  const parts = id.slice('claude-'.length).split('-').filter((p) => !/^\d{8}$/.test(p));
  const family = parts.find((p) => !/^\d+$/.test(p));
  if (!family) return id;
  const version = parts.filter((p) => /^\d+$/.test(p)).join('.');
  return `${family.charAt(0).toUpperCase()}${family.slice(1)} ${version}`.trim();
}

/** "2h 13m left", "13m left": minutes rounded up, so the last minute still reads "1m left". */
export function left(ms: number): string {
  const m = Math.max(0, Math.ceil(ms / 60_000));
  return m >= 60 ? `${Math.floor(m / 60)}h ${m % 60}m left` : `${m}m left`;
}
