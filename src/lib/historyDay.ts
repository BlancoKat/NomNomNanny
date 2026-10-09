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
