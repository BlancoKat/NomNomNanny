import assert from 'node:assert/strict';
import { test } from 'node:test';
import { foodsForDate, openHistoryDay, selectToday } from '../src/lib/historyDay.ts';

test('a history row opens that day on the log', () => {
  assert.deepEqual(openHistoryDay('2026-10-03'), { date: '2026-10-03', tab: 'today' });
});

test('a history row is not a free-form label', () => {
  assert.throws(() => openHistoryDay('October 3'));
});

test('Today nav returns to the local day after a history day and reloads that day’s foods', () => {
  const foods = [
    { log_date: '2026-10-10', description: 'oats' },
    { log_date: '2026-10-03', description: 'soup' },
    { log_date: '2026-10-03', description: 'milk' },
  ];
  const localToday = '2026-10-10';

  const opened = openHistoryDay('2026-10-03');
  assert.equal(opened.tab, 'today');
  assert.equal(opened.date, '2026-10-03');
  assert.deepEqual(
    foodsForDate(foods, opened.date).map((entry) => entry.description),
    ['soup', 'milk']
  );

  const back = selectToday(localToday);
  assert.equal(back.tab, 'today');
  assert.equal(back.date, localToday);
  assert.notEqual(back.date, opened.date);
  assert.deepEqual(
    foodsForDate(foods, back.date).map((entry) => entry.description),
    ['oats']
  );
});
