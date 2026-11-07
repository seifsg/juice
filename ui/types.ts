/** Snapshot shapes as the Rust core serializes them (camelCase). */

export interface Tokens {
  input: number;
  output: number;
  cacheWrite: number;
  cacheRead: number;
}

export interface ModelRow {
  model: string;
  tokens: Tokens;
  /** USD of the priced responses. */
  cost: number;
  /** Some responses had no known price, so `cost` is a lower bound. */
  unpriced: boolean;
}

export interface Period {
  tokens: number;
  cost: number;
  unpriced: boolean;
  /** Sorted by cost, then tokens. */
  models: ModelRow[];
}

export interface Snapshot {
  window: Period;
  /** When the current 5h window resets (ms since epoch); null when idle. */
  windowEnd: number | null;
  today: Period;
  /** Usage lines that could not be read. */
  unreadable: number;
  root: string;
  rootFound: boolean;
}
