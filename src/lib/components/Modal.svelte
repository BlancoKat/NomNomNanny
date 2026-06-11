<script lang="ts">
  interface Props {
    open: boolean;
    onClose: () => void;
    title?: string;
    children?: any;
  }

  let { open = $bindable(false), onClose, title, children }: Props = $props();

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) onClose();
  }

  function stopPropagation(e: MouseEvent) {
    e.stopImmediatePropagation();
  }
</script>

<svelte:window onkeydown={handleKey} />

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" onclick={handleBackdrop}>
    <div class="bg-white rounded-3xl shadow-2xl w-full max-w-2xl overflow-hidden modal" onclick={stopPropagation}>
      {#if title}
        <div class="px-6 pt-5 pb-4 border-b flex items-center justify-between">
          <div class="font-semibold text-lg">{title}</div>
          <button onclick={onClose} class="text-slate-400 hover:text-slate-600 text-2xl leading-none pb-1">×</button>
        </div>
      {/if}
      <div class="p-6">
        {@render children?.()}
      </div>
    </div>
  </div>
{/if}