import assert from 'node:assert/strict';
import { test } from 'node:test';
import { openHistoryDay } from '../src/lib/historyDay.ts';

test('a history row opens that day on the log', () => {
  assert.deepEqual(openHistoryDay('2026-10-03'), { date: '2026-10-03', tab: 'today' });
});

test('a history row is not a free-form label', () => {
  assert.throws(() => openHistoryDay('October 3'));
});
