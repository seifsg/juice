import { test } from 'node:test';
import assert from 'node:assert/strict';
import { left, model, money, tokens } from './format.ts';

test('money, tokens, model names and countdown', () => {
  assert.deepEqual([money(3.421), money(99.994), money(99.996), money(0.5, true)], ['$3.42', '$99.99', '$100', '$0.50+']);
  assert.deepEqual([950, 9_949, 9_960, 340_000, 999_499, 999_500, 1.2e9].map(tokens), ['950', '9.9k', '10k', '340k', '999k', '1.0M', '1.2B']);
  assert.deepEqual(
    ['claude-opus-5-5', 'claude-haiku-4-5-20251001', 'claude-3-5-sonnet-20241022', 'claude-fable-5-1', 'gpt-x', 'claude-'].map(model),
    ['Opus 5.5', 'Haiku 4.5', 'Sonnet 3.5', 'Fable 5.1', 'gpt-x', 'claude-'],
  );
  assert.deepEqual([left(30_000), left(13 * 60_000), left(133 * 60_000), left(-5)], ['1m left', '13m left', '2h 13m left', '0m left']);
});
