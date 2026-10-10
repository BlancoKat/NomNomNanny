/**
 * Opening a history row shows that day's log.
 *
 * v0.1.0 already jumped to the Today tab, but the click never reloaded
 * entries. The page effect reads the date only after its first await, so
 * changing the date alone left the previously loaded foods on screen.
 */
export function openHistoryDay(logDate: string): { date: string; tab: 'today' } {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(logDate)) {
    throw new Error(`not a calendar day: ${logDate}`);
  }
  return { date: logDate, tab: 'today' };
}

/**
 * The Today item in the nav.
 *
 * Opening a history day already leaves the Today tab selected, so a click
 * that only switches the tab does nothing and the log stays on that older
 * date. Today always means the local calendar day, and the foods for that
 * day are loaded again.
 */
export function selectToday(localToday: string): { date: string; tab: 'today' } {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(localToday)) {
    throw new Error(`not a calendar day: ${localToday}`);
  }
  return { date: localToday, tab: 'today' };
}

export type DiaryFood = { log_date: string; description: string };

export function foodsForDate(entries: DiaryFood[], date: string): DiaryFood[] {
  return entries.filter((entry) => entry.log_date === date);
}
