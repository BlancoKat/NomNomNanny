<script lang="ts">
  interface Props {
    label: string;
    current: number;
    goal: number;
    unit: string;
    color: string; // tailwind color or hex
    icon?: any;
  }

  let { label, current, goal, unit, color, icon: Icon }: Props = $props();

  const pct = $derived(goal > 0 ? Math.min(100, Math.max(0, (current / goal) * 100)) : 0);
  const remaining = $derived(Math.max(0, goal - current));

  const radius = 42;
  const circumference = 2 * Math.PI * radius;
  const strokeDashoffset = $derived(circumference - (pct / 100) * circumference);
</script>

<div class="bg-white border border-slate-200 rounded-3xl p-4 flex flex-col items-center shadow-sm hover:shadow transition-shadow">
  <div class="relative w-[92px] h-[92px] flex items-center justify-center">
    <!-- Background circle -->
    <svg class="w-[92px] h-[92px] -rotate-90" viewBox="0 0 100 100">
      <circle
        cx="50"
        cy="50"
        r={radius}
        fill="none"
        stroke="#f1f5f9"
        stroke-width="8"
      />
      <!-- Progress -->
      <circle
        cx="50"
        cy="50"
        r={radius}
        fill="none"
        stroke={color}
        stroke-width="8"
        stroke-linecap="round"
        stroke-dasharray={circumference}
        stroke-dashoffset={strokeDashoffset}
        style="transition: stroke-dashoffset 0.35s cubic-bezier(0.4, 0, 0.2, 1);"
      />
    </svg>

    <!-- Center content -->
    <div class="absolute inset-0 flex flex-col items-center justify-center text-center">
      <div class="text-[21px] font-semibold tabular-nums leading-none" style="color: {color}">
        {current.toFixed(unit === 'kcal' ? 0 : 1)}
      </div>
      <div class="text-[10px] text-slate-400 mt-0.5">/ {goal} {unit}</div>
    </div>
  </div>

  <div class="mt-2.5 text-center">
    <div class="flex items-center justify-center gap-1.5 text-sm font-medium text-slate-700">
      {#if Icon}<span style="color: {color}"><svelte:component this={Icon} class="w-3.5 h-3.5" /></span>{/if}
      {label}
    </div>
    <div class="text-[11px] mt-0.5" style="color: {pct >= 100 ? '#16a34a' : '#64748b'}">
      {pct >= 100 ? 'Goal met ✓' : `${remaining.toFixed(unit === 'kcal' ? 0 : 1)} ${unit} left`}
    </div>
  </div>
</div>