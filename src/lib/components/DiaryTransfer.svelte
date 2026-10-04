<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { Download, Upload } from 'lucide-svelte';
  import Modal from '$lib/components/Modal.svelte';

  interface Goal {
    calories_kcal: number;
    protein_g: number;
    fat_g: number;
    carbs_g: number;
    fiber_g: number;
    hydration_oz: number;
  }

  interface DiarySummary {
    entry_count: number;
    first_date: string | null;
    last_date: string | null;
    custom_food_count: number;
    custom_food_names: string[];
    custom_foods_omitted: number;
    goals: Goal;
  }

  interface DiaryImportPreview {
    device: DiarySummary;
    file: DiarySummary;
  }

  let { onImported }: { onImported: () => void | Promise<void> } = $props();

  let busy = $state(false);
  let message = $state('');
  let error = $state('');
  let showConfirm = $state(false);
  let pendingContents = $state('');
  let preview = $state<DiaryImportPreview | null>(null);

  function errorText(cause: unknown): string {
    if (typeof cause === 'string') return cause;
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message);
    }
    return String(cause ?? 'Something went wrong');
  }

  function cancelled(cause: unknown): boolean {
    return /cancel/i.test(errorText(cause));
  }

  function amount(value: number): string {
    if (!Number.isFinite(value)) return String(value);
    const rounded = Math.round(value * 10) / 10;
    return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
  }

  function goalLine(goals: Goal): string {
    return `${amount(goals.calories_kcal)} kcal, ${amount(goals.protein_g)} g protein, ${amount(goals.fat_g)} g fat, ${amount(goals.carbs_g)} g carbs, ${amount(goals.fiber_g)} g fiber, ${amount(goals.hydration_oz)} fl oz water`;
  }

  function entryLine(summary: DiarySummary): string {
    if (summary.entry_count === 0) return 'No log entries';
    const noun = summary.entry_count === 1 ? 'log entry' : 'log entries';
    if (!summary.first_date || !summary.last_date) return `${summary.entry_count} ${noun}`;
    const range = summary.first_date === summary.last_date
      ? summary.first_date
      : `${summary.first_date} to ${summary.last_date}`;
    return `${summary.entry_count} ${noun} (${range})`;
  }

  function foodLine(summary: DiarySummary): string {
    if (summary.custom_food_count === 0) return 'No custom foods';
    const noun = summary.custom_food_count === 1 ? 'custom food' : 'custom foods';
    if (summary.custom_food_names.length === 0) return `${summary.custom_food_count} ${noun}`;
    const extra = summary.custom_foods_omitted > 0 ? `, and ${summary.custom_foods_omitted} more` : '';
    return `${summary.custom_food_count} ${noun}: ${summary.custom_food_names.join(', ')}${extra}`;
  }

  function clearNoticeLater(text: string) {
    message = text;
    error = '';
    setTimeout(() => {
      if (message === text) message = '';
    }, 2500);
  }

  async function exportDiary() {
    if (busy) return;
    busy = true;
    error = '';
    try {
      const contents = await invoke<string>('export_diary_cmd');
      const path = await save({
        title: 'Export NomNom diary',
        defaultPath: 'nomnom-diary.json',
        filters: [{ name: 'NomNom diary', extensions: ['json'] }],
      });
      if (!path) return;
      await invoke('write_diary_file_cmd', { path, contents });
      clearNoticeLater('Diary exported');
    } catch (cause) {
      if (!cancelled(cause)) error = errorText(cause);
    } finally {
      busy = false;
    }
  }

  function closeConfirm() {
    showConfirm = false;
    pendingContents = '';
    preview = null;
  }

  async function chooseImport() {
    if (busy) return;
    busy = true;
    error = '';
    try {
      const path = await open({
        title: 'Import NomNom diary',
        multiple: false,
        directory: false,
        filters: [{ name: 'NomNom diary', extensions: ['json'] }],
      });
      if (!path || Array.isArray(path)) return;
      const contents = await invoke<string>('read_diary_file_cmd', { path });
      preview = await invoke<DiaryImportPreview>('preview_diary_import_cmd', { contents });
      pendingContents = contents;
      showConfirm = true;
    } catch (cause) {
      if (!cancelled(cause)) error = errorText(cause);
    } finally {
      busy = false;
    }
  }

  async function confirmImport() {
    if (busy || !pendingContents || !preview) return;
    busy = true;
    error = '';
    try {
      await invoke('import_diary_cmd', { contents: pendingContents, confirm: true });
      closeConfirm();
      await onImported();
      clearNoticeLater('Diary imported');
    } catch (cause) {
      error = errorText(cause);
    } finally {
      busy = false;
    }
  }
</script>

<section class="bg-white border rounded-3xl p-4 sm:p-6 space-y-3">
  <div class="font-semibold">Move this diary</div>
  <p class="text-xs text-slate-500 leading-relaxed">
    Export one file and open it on the other device. The file contains goals, log entries, and custom foods.
    It leaves out the USDA cache and your API key. Import replaces the diary already on this device. It does not merge the two copies.
  </p>
  <div class="flex flex-col sm:flex-row gap-2">
    <button
      type="button"
      onclick={exportDiary}
      disabled={busy}
      class="min-h-11 px-4 py-2.5 border border-emerald-200 text-emerald-700 rounded-2xl text-sm font-medium inline-flex items-center justify-center gap-2 disabled:opacity-60"
    >
      <Download class="w-4 h-4" />
      Export diary
    </button>
    <button
      type="button"
      onclick={chooseImport}
      disabled={busy}
      class="min-h-11 px-4 py-2.5 border border-slate-200 rounded-2xl text-sm font-medium inline-flex items-center justify-center gap-2 disabled:opacity-60"
    >
      <Upload class="w-4 h-4" />
      Import diary
    </button>
  </div>
  {#if message}<div class="text-xs text-emerald-700">{message}</div>{/if}
  {#if error}<div class="text-xs text-rose-600 break-words">{error}</div>{/if}
</section>

<Modal bind:open={showConfirm} onClose={closeConfirm} title="Replace diary on this device?">
  {#if preview}
    <div class="space-y-4 text-sm">
      <p class="text-slate-600">
        This replaces the diary on this device with the file. It does not merge them.
        The USDA cache and API key on this device stay as they are.
      </p>
      <div class="grid gap-3 sm:grid-cols-2">
        <div class="border rounded-2xl p-3 bg-slate-50 space-y-1">
          <div class="text-xs font-semibold uppercase tracking-wide text-slate-500">On this device now</div>
          <div>{goalLine(preview.device.goals)}</div>
          <div>{entryLine(preview.device)}</div>
          <div>{foodLine(preview.device)}</div>
        </div>
        <div class="border border-amber-200 rounded-2xl p-3 bg-amber-50 space-y-1">
          <div class="text-xs font-semibold uppercase tracking-wide text-amber-700">File will replace it with</div>
          <div>{goalLine(preview.file.goals)}</div>
          <div>{entryLine(preview.file)}</div>
          <div>{foodLine(preview.file)}</div>
        </div>
      </div>
      {#if error}<div class="text-xs text-rose-600 break-words">{error}</div>{/if}
      <div class="flex flex-col-reverse sm:flex-row sm:justify-end gap-2">
        <button type="button" onclick={closeConfirm} class="min-h-11 px-4 py-2.5 rounded-2xl text-sm">Cancel</button>
        <button
          type="button"
          onclick={confirmImport}
          disabled={busy}
          class="min-h-11 px-4 py-2.5 bg-rose-600 text-white rounded-2xl text-sm font-medium disabled:opacity-60"
        >
          {busy ? 'Replacing…' : 'Replace diary on this device'}
        </button>
      </div>
    </div>
  {/if}
</Modal>
