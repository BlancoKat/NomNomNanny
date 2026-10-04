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

  // The app shell uses overflow:hidden. Move the dialog to document.body so a
  // phone-height sheet is not clipped and can scroll inside the safe area.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }
</script>

<svelte:window onkeydown={handleKey} />

{#if open}
  <div
    class="modal-layer fixed inset-0 z-50 flex items-end sm:items-center justify-center bg-black/40"
    use:portal
    role="presentation"
    onclick={handleBackdrop}
    onkeydown={(event) => { if (event.key === 'Escape') onClose(); }}
  >
    <div
      class="bg-white shadow-2xl w-full max-w-2xl max-h-full overflow-y-auto overscroll-contain rounded-t-3xl sm:rounded-3xl modal"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby={title ? 'modal-title' : undefined}
      onclick={stopPropagation}
      onkeydown={(event) => { if (event.key === 'Escape') onClose(); }}
    >
      {#if title}
        <div class="sticky top-0 z-10 bg-white px-4 sm:px-6 pt-5 pb-4 border-b flex items-center justify-between gap-3">
          <div id="modal-title" class="font-semibold text-lg">{title}</div>
          <button
            type="button"
            onclick={onClose}
            class="min-w-11 min-h-11 text-slate-400 hover:text-slate-600 text-2xl leading-none"
            aria-label="Close"
          >×</button>
        </div>
      {/if}
      <div class="p-4 sm:p-6">
        {@render children?.()}
      </div>
    </div>
  </div>
{/if}
