<script lang="ts">
  import { addLocalDays, localDateString } from '$lib/localDate';

  interface Props {
    date: string;
    onChange: (newDate: string) => void;
  }

  let { date, onChange }: Props = $props();

  const today = localDateString();

  function change(delta: number) {
    onChange(addLocalDays(date, delta));
  }

  function goToday() {
    onChange(today);
  }

  const formatted = $derived(
    new Date(date + 'T00:00:00').toLocaleDateString(undefined, {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
    })
  );

  const shortFormatted = $derived(
    new Date(date + 'T00:00:00').toLocaleDateString(undefined, {
      weekday: 'short',
      month: 'short',
      day: 'numeric',
    })
  );
</script>

<div class="flex items-center gap-2 min-w-0">
  <button
    type="button"
    onclick={() => change(-1)}
    class="w-11 h-11 shrink-0 flex items-center justify-center rounded-2xl border border-slate-200 hover:bg-slate-50 active:bg-slate-100 text-slate-500"
    aria-label="Previous day"
  >
    ←
  </button>

  <div class="flex flex-col items-center min-w-0">
    <div class="font-semibold text-base sm:hidden tracking-tight text-slate-800 text-center">{shortFormatted}</div>
    <div class="font-semibold text-lg tracking-tight text-slate-800 hidden sm:block text-center">{formatted}</div>
    <button
      onclick={goToday}
      disabled={date === today}
      class="text-[11px] text-emerald-600 hover:text-emerald-700 disabled:text-slate-400 disabled:cursor-default -mt-0.5"
    >
      {date === today ? 'Today' : 'Jump to today'}
    </button>
  </div>

  <button
    type="button"
    onclick={() => change(1)}
    class="w-11 h-11 shrink-0 flex items-center justify-center rounded-2xl border border-slate-200 hover:bg-slate-50 active:bg-slate-100 text-slate-500"
    aria-label="Next day"
  >
    →
  </button>
</div>