/** Calendar date in the device's local timezone, as YYYY-MM-DD. */
export function localDateString(date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

/**
 * Add days to a YYYY-MM-DD date on the local calendar.
 *
 * The released stepper did `new Date("YYYY-MM-DD")` (UTC midnight), then
 * `setDate`, then `toISOString().slice(0, 10)`. On the spring-forward date
 * that round trip returns the same day, so the next button does nothing
 * until the app is restarted on a later date. Building the date from the
 * year, month, and day keeps the calendar day and the stored string together.
 */
export function addLocalDays(isoDate: string, delta: number): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(isoDate);
  if (!match) return isoDate;
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  date.setDate(date.getDate() + delta);
  return localDateString(date);
}

/**
 * Move the open day forward when the local calendar day changes while the
 * app is still running. A day she navigated to on purpose stays put.
 */
export function rollDisplayedDay(
  currentDate: string,
  previousToday: string,
  now = new Date()
): { today: string; currentDate: string; rolled: boolean } {
  const today = localDateString(now);
  if (today === previousToday) {
    return { today, currentDate, rolled: false };
  }
  const viewingToday = currentDate === previousToday;
  return {
    today,
    currentDate: viewingToday ? today : currentDate,
    rolled: viewingToday,
  };
}
