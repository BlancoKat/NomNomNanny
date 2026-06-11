<script lang="ts">
  interface Props {
    date: string;
    onChange: (newDate: string) => void;
  }

  let { date, onChange }: Props = $props();

  const today = new Date().toISOString().slice(0, 10);

  function change(delta: number) {
    const d = new Date(date);
    d.setDate(d.getDate() + delta);
    onChange(d.toISOString().slice(0, 10));
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
</script>

<div class="flex items-center gap-2">
  <button
    onclick={() => change(-1)}
    class="w-9 h-9 flex items-center justify-center rounded-2xl border border-slate-200 hover:bg-slate-50 active:bg-slate-100 text-slate-500"
    aria-label="Previous day"
  >
    ←
  </button>

  <div class="flex flex-col items-center min-w-[210px]">
    <div class="font-semibold text-lg tracking-tight text-slate-800">{formatted}</div>
    <button
      onclick={goToday}
      disabled={date === today}
      class="text-[11px] text-emerald-600 hover:text-emerald-700 disabled:text-slate-400 disabled:cursor-default -mt-0.5"
    >
      {date === today ? 'Today' : 'Jump to today'}
    </button>
  </div>

  <button
    onclick={() => change(1)}
    class="w-9 h-9 flex items-center justify-center rounded-2xl border border-slate-200 hover:bg-slate-50 active:bg-slate-100 text-slate-500"
    aria-label="Next day"
  >
    →
  </button>
</div>