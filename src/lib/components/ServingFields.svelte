<script lang="ts">
  let {
    amount = $bindable(100),
    unit = $bindable('g'),
    density = $bindable(''),
    showDensity = true,
  }: {
    amount: number;
    unit: string;
    density?: string;
    showDensity?: boolean;
  } = $props();

  const volume = $derived(unit === 'ml' || unit === 'fl oz');
</script>

<div class="flex flex-wrap items-end gap-2">
  <label class="block">
    <span class="text-[10px] text-slate-500">Amount</span>
    <input type="number" min="0" step="0.1" bind:value={amount} class="mt-0.5 w-24 border rounded-xl px-3 py-2 text-sm" />
  </label>
  <label class="block">
    <span class="text-[10px] text-slate-500">Unit</span>
    <select bind:value={unit} class="mt-0.5 border rounded-xl px-3 py-2 text-sm bg-white">
      <option value="g">g</option>
      <option value="ml">mL</option>
      <option value="oz">oz (weight)</option>
      <option value="fl oz">fl oz</option>
    </select>
  </label>
  {#if showDensity && volume}
    <label class="block">
      <span class="text-[10px] text-slate-500">Density g/mL</span>
      <input
        type="number"
        min="0"
        step="0.01"
        bind:value={density}
        placeholder="1"
        class="mt-0.5 w-24 border rounded-xl px-3 py-2 text-sm"
      />
    </label>
  {/if}
</div>
