import assert from 'node:assert/strict';
import { test } from 'node:test';
import { addLocalDays, localDateString, rollDisplayedDay } from '../src/lib/localDate.ts';

/** The v0.1.1 DateNavigator step. Kept here so the regression stays visible. */
function legacyIsoStep(isoDate: string, delta: number): string {
  const d = new Date(isoDate);
  d.setDate(d.getDate() + delta);
  return d.toISOString().slice(0, 10);
}

const steps: Array<[string, string]> = [
  ['2026-03-08', '2026-03-09'],
  ['2026-03-07', '2026-03-08'],
  ['2026-11-01', '2026-11-02'],
  ['2026-10-03', '2026-10-04'],
  ['2026-03-29', '2026-03-30'],
  ['2026-10-09', '2026-10-10'],
  ['2026-01-31', '2026-02-01'],
  ['2026-12-31', '2027-01-01'],
];

test('the next calendar day is always a different date', () => {
  for (const [start, next] of steps) {
    assert.equal(addLocalDays(start, 1), next);
    assert.notEqual(addLocalDays(start, 1), start);
    assert.equal(addLocalDays(next, -1), start);
  }
});

test('the released stepper is stuck on the spring-forward date in this timezone', () => {
  const tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const stuck: Record<string, string> = {
    'America/Chicago': '2026-03-08',
    'America/New_York': '2026-03-08',
    'Europe/Berlin': '2026-03-29',
    'Australia/Sydney': '2026-10-03',
  };
  const start = stuck[tz];
  if (!start) return;
  assert.equal(legacyIsoStep(start, 1), start);
  assert.notEqual(addLocalDays(start, 1), start);
});

test('a local evening west of UTC is not the UTC date the old startup used', () => {
  const evening = new Date(2026, 9, 9, 22, 0, 0);
  if (evening.getTimezoneOffset() <= 120) return;
  assert.equal(localDateString(evening), '2026-10-09');
  assert.notEqual(evening.toISOString().slice(0, 10), '2026-10-09');
});

test('the open day rolls forward at local midnight without a restart', () => {
  const justAfterMidnight = new Date(2026, 9, 10, 0, 5, 0);
  const rolled = rollDisplayedDay('2026-10-09', '2026-10-09', justAfterMidnight);
  assert.equal(rolled.today, '2026-10-10');
  assert.equal(rolled.currentDate, '2026-10-10');
  assert.equal(rolled.rolled, true);

  const lookingAtAnOlderDay = rollDisplayedDay('2026-10-01', '2026-10-09', justAfterMidnight);
  assert.equal(lookingAtAnOlderDay.currentDate, '2026-10-01');
  assert.equal(lookingAtAnOlderDay.rolled, false);

  const sameDay = rollDisplayedDay('2026-10-09', '2026-10-09', new Date(2026, 9, 9, 18, 0, 0));
  assert.equal(sameDay.currentDate, '2026-10-09');
  assert.equal(sameDay.rolled, false);
});

test('tomorrow is not capped', () => {
  const today = localDateString(new Date(2026, 9, 9, 12, 0, 0));
  assert.equal(addLocalDays(today, 1), '2026-10-10');
});
