<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { Store } from '@tauri-apps/plugin-store';
  import { Apple, Droplet, Plus, Trash2, Calendar, History, Target, Search, X } from 'lucide-svelte';
  import MacroRing from '$lib/components/MacroRing.svelte';
  import DateNavigator from '$lib/components/DateNavigator.svelte';
  import Modal from '$lib/components/Modal.svelte';

  interface Goal { calories_kcal: number; protein_g: number; fat_g: number; carbs_g: number; fiber_g: number; hydration_oz: number; }
  interface IntakeEntry { id: number; log_date: string; description: string; amount: number; unit: string; calories_kcal: number; protein_g: number; fat_g: number; carbs_g: number; fiber_g: number; fluid_oz: number; meal?: string; source: string; }
  interface DailyTotals { log_date: string; calories_kcal: number; protein_g: number; fat_g: number; carbs_g: number; fiber_g: number; fluid_oz: number; entry_count: number; }

  interface UsdaHit { fdc_id: number; description: string; data_type?: string; brand_owner?: string; }
  interface FoodPortion { label: string; grams: number; }
  interface MacroNutrients { kcal: number; protein: number; fat: number; carbs: number; fiber: number; }
  interface CachedFood { fdc_id: number; description: string; nutrients_per_100g: MacroNutrients; portions: FoodPortion[]; }

  let today = new Date().toISOString().slice(0, 10);
  let currentDate = $state(today);
  let activeTab = $state<'today' | 'history' | 'goals'>('today');

  let goals = $state<Goal | null>(null);
  let entries = $state<IntakeEntry[]>([]);
  let totals = $state<DailyTotals | null>(null);
  let averages7 = $state<any>(null);
  let history = $state<any[]>([]);

  let waterOz = $state(8);
  let showCustomForm = $state(false);
  let custom = $state({ name: 'Greek yogurt', kcal: 100, protein: 10, fat: 2, carbs: 8, fiber: 0 });

  // Food search
  let showFoodModal = $state(false);

  // Custom foods
  interface CustomFood {
    id: number;
    name: string;
    kcal_per_100g: number;
    protein_per_100g: number;
    fat_per_100g: number;
    carbs_per_100g: number;
    fiber_per_100g: number;
  }

  let customFoods = $state<CustomFood[]>([]);
  let showMyFoodsModal = $state(false);

  // Save current manual entry as custom food flow
  let showSaveCustomDialog = $state(false);
  let saveCustomGrams = $state(100);

  // Advanced logging from My Foods (pick food + quantity + optional overrides)
  let activeLogFoodId = $state<number | null>(null);
  let logGrams = $state(100);
  let useNutrientOverride = $state(false);
  let overrideNutrients = $state({ kcal: 0, protein: 0, fat: 0, carbs: 0, fiber: 0 });
  let foodQuery = $state('');
  let searchResults = $state<UsdaHit[]>([]);
  let isSearching = $state(false);
  let selectedFood = $state<CachedFood | null>(null);
  let selectedPortionIndex = $state(0);
  let portionMultiplier = $state(1.0);
  let apiKey = $state('');
  let store: Store | null = null;

  let status = $state('');
  let isLoading = $state(false);

  // Search-specific state
  let searchError = $state('');

  async function initStore() {
    if (!store) {
      store = await Store.load('settings.json');
      const savedKey = await store.get<string>('usdaApiKey');
      if (savedKey) apiKey = savedKey;
    }
  }

  async function saveApiKey() {
    await initStore();
    if (store) {
      await store.set('usdaApiKey', apiKey.trim());
      await store.save();
      status = 'API key saved';
      setTimeout(() => { if (status === 'API key saved') status = ''; }, 1500);
    }
  }

  // Custom Foods management
  async function loadCustomFoods() {
    customFoods = await invoke('get_custom_foods_cmd');
  }

  async function saveCurrentAsCustomFood() {
    if (!custom.name.trim()) return;
    showSaveCustomDialog = true;
    saveCustomGrams = 100; // sensible default
  }

  async function confirmSaveCurrentAsCustomFood() {
    if (!custom.name.trim() || saveCustomGrams <= 0) {
      showSaveCustomDialog = false;
      return;
    }

    const factor = 100 / saveCustomGrams;

    const food: CustomFood = {
      id: 0,
      name: custom.name.trim(),
      kcal_per_100g: custom.kcal * factor,
      protein_per_100g: custom.protein * factor,
      fat_per_100g: custom.fat * factor,
      carbs_per_100g: custom.carbs * factor,
      fiber_per_100g: custom.fiber * factor,
    };

    await invoke('save_custom_food_cmd', { food });
    await loadCustomFoods();
    showSaveCustomDialog = false;
    status = 'Saved to My Foods';
    setTimeout(() => { if (status === 'Saved to My Foods') status = ''; }, 1500);
  }

  async function updateCustomFood(food: CustomFood) {
    await invoke('save_custom_food_cmd', { food });
    await loadCustomFoods();
  }

  async function deleteCustomFood(id: number) {
    await invoke('delete_custom_food_cmd', { id });
    await loadCustomFoods();
  }

  async function logFromCustomFood(food: CustomFood, grams: number) {
    if (grams <= 0) return;

    const factor = grams / 100;
    const scaled = {
      kcal: food.kcal_per_100g * factor,
      protein: food.protein_per_100g * factor,
      fat: food.fat_per_100g * factor,
      carbs: food.carbs_per_100g * factor,
      fiber: food.fiber_per_100g * factor,
    };

    await invoke('log_intake_cmd', {
      input: {
        log_date: currentDate,
        description: food.name,
        amount: grams,
        unit: 'g',
        grams: grams,
        calories_kcal: scaled.kcal,
        protein_g: scaled.protein,
        fat_g: scaled.fat,
        carbs_g: scaled.carbs,
        fiber_g: scaled.fiber,
        fluid_oz: 0,
        meal: 'Snack',
        source: 'custom'
      }
    });

    await loadAll();
    status = `Logged ${grams}g of ${food.name}`;
    setTimeout(() => { if (status.startsWith('Logged')) status = ''; }, 1500);
  }

  function startLoggingFood(food: CustomFood) {
    activeLogFoodId = food.id;
    logGrams = 100;
    useNutrientOverride = false;

    // Pre-fill overrides with calculated values for 100g
    const factor = 100 / 100;
    overrideNutrients = {
      kcal: food.kcal_per_100g * factor,
      protein: food.protein_per_100g * factor,
      fat: food.fat_per_100g * factor,
      carbs: food.carbs_per_100g * factor,
      fiber: food.fiber_per_100g * factor,
    };
  }

  async function confirmLogFromMyFoods(food: CustomFood) {
    const g = logGrams;
    if (g <= 0) return;

    let finalValues;

    if (useNutrientOverride) {
      // Use the overridden values as totals for the entered grams
      finalValues = { ...overrideNutrients };
    } else {
      // Scale from the saved per-100g profile
      const factor = g / 100;
      finalValues = {
        kcal: food.kcal_per_100g * factor,
        protein: food.protein_per_100g * factor,
        fat: food.fat_per_100g * factor,
        carbs: food.carbs_per_100g * factor,
        fiber: food.fiber_per_100g * factor,
      };
    }

    await invoke('log_intake_cmd', {
      input: {
        log_date: currentDate,
        description: food.name,
        amount: g,
        unit: 'g',
        grams: g,
        calories_kcal: finalValues.kcal,
        protein_g: finalValues.protein,
        fat_g: finalValues.fat,
        carbs_g: finalValues.carbs,
        fiber_g: finalValues.fiber,
        fluid_oz: 0,
        meal: 'Snack',
        source: 'custom'
      }
    });

    await loadAll();
    activeLogFoodId = null;
    status = `Logged ${g}g of ${food.name}`;
    setTimeout(() => { if (status.startsWith('Logged')) status = ''; }, 1500);
  }

  async function loadAll() {
    isLoading = true;
    try {
      goals = await invoke('get_goals_cmd');
      entries = await invoke('get_entries_for_date_cmd', { date: currentDate });
      totals = await invoke('get_daily_totals_cmd', { date: currentDate });
      averages7 = await invoke('get_averages_cmd', { days: 7 });
      history = await invoke('get_history_summary_cmd', { limit: 14 });
    } catch (e: any) { status = 'Error: ' + (e?.message || e); }
    finally { isLoading = false; }
  }

  async function logWater() {
    if (waterOz <= 0) return;
    await invoke('log_intake_cmd', { input: { log_date: currentDate, description: 'Water', amount: waterOz, unit: 'fl oz', grams: null, calories_kcal: 0, protein_g: 0, fat_g: 0, carbs_g: 0, fiber_g: 0, fluid_oz: waterOz, meal: null, source: 'water' } });
    await loadAll();
  }

  async function logCustomEntry() {
    await invoke('log_intake_cmd', { input: { log_date: currentDate, description: custom.name, amount: 1, unit: 'serving', grams: 100, calories_kcal: custom.kcal, protein_g: custom.protein, fat_g: custom.fat, carbs_g: custom.carbs, fiber_g: custom.fiber, fluid_oz: 0, meal: 'Snack', source: 'custom' } });
    showCustomForm = false;
    await loadAll();
  }

  async function deleteEntry(id: number) { await invoke('delete_entry_cmd', { id }); await loadAll(); }

  let searchTimeout: any;

  async function performSearch(immediate = false) {
    const query = foodQuery.trim();
    if (!query) { 
      searchResults = []; 
      searchError = ''; 
      return; 
    }
    if (query.length < 2) return;

    isSearching = true;
    searchError = '';

    try {
      const key = apiKey.trim() || 'DEMO_KEY';
      searchResults = await invoke('search_usda_foods_cmd', { query, apiKey: key });
    } catch (e: any) {
      searchResults = [];
      const msg = e?.message || String(e) || 'Unknown error';
      searchError = msg.includes('rate') || msg.includes('429') 
        ? 'USDA rate limit reached. Wait a minute or use a valid API key.' 
        : `Search failed: ${msg}`;
      console.error('USDA search error:', e);
    } finally {
      isSearching = false;
    }
  }

  function onSearchInput() { 
    clearTimeout(searchTimeout); 
    searchTimeout = setTimeout(() => performSearch(), 300); 
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      clearTimeout(searchTimeout);
      performSearch(true);
    }
  }

  let detailLoading = $state(false);

  async function selectFood(hit: UsdaHit) {
    detailLoading = true;
    searchError = '';

    try {
      const key = apiKey.trim() || 'DEMO_KEY';
      selectedFood = await invoke('fetch_usda_food_details_cmd', { fdcId: hit.fdc_id, apiKey: key });
      selectedPortionIndex = 0; 
      portionMultiplier = 1;
    } catch (e: any) {
      const msg = e?.message || String(e) || 'Unknown error';
      searchError = `Failed to load food details: ${msg}`;
      console.error('selectFood error:', e);
      selectedFood = null;
    } finally {
      detailLoading = false;
    }
  }

  let previewNutrients = $state<MacroNutrients | null>(null);
  let previewLoading = $state(false);

  $effect(() => {
    if (!selectedFood) {
      previewNutrients = null;
      previewLoading = false;
      return;
    }

    const p = selectedFood.portions[selectedPortionIndex];
    const g = (p.grams > 0 ? p.grams : 100) * portionMultiplier;

    previewLoading = true;
    invoke('scale_nutrients_cmd', {
      nutrients: selectedFood.nutrients_per_100g,
      grams: g
    })
      .then((res: any) => {
        previewNutrients = res;
      })
      .catch(() => {
        previewNutrients = null;
      })
      .finally(() => {
        previewLoading = false;
      });
  });

  let logging = $state(false);

  async function logSelectedFood() {
    if (!selectedFood || logging) return;

    logging = true;

    try {
      const p = selectedFood.portions[selectedPortionIndex];
      const g = (p.grams > 0 ? p.grams : 100) * portionMultiplier;

      const scaled: MacroNutrients = await invoke('scale_nutrients_cmd', {
        nutrients: selectedFood.nutrients_per_100g,
        grams: g
      });

      await invoke('log_intake_cmd', {
        input: {
          log_date: currentDate,
          fdc_id: selectedFood.fdc_id,
          description: selectedFood.description,
          amount: portionMultiplier,
          unit: p.label,
          grams: g,
          calories_kcal: scaled.kcal,
          protein_g: scaled.protein,
          fat_g: scaled.fat,
          carbs_g: scaled.carbs,
          fiber_g: scaled.fiber,
          fluid_oz: 0,
          meal: null,
          source: 'usda'
        }
      });

      closeFoodModal();
      await loadAll();
    } catch (e: any) {
      searchError = `Failed to log food: ${e?.message || e}`;
      console.error('logSelectedFood error:', e);
    } finally {
      logging = false;
    }
  }

  function closeFoodModal() { 
    showFoodModal = false; 
    foodQuery = ''; 
    searchResults = []; 
    searchError = '';
    selectedFood = null; 
    portionMultiplier = 1; 
    detailLoading = false;
  }
  function openFoodSearch() { showFoodModal = true; setTimeout(() => (document.getElementById('food-search-input') as HTMLInputElement)?.focus(), 60); }

  function changeDate(d: string) { currentDate = d; loadAll(); }

  const progress = $derived(goals && totals ? {
    kcal: { cur: totals.calories_kcal, goal: goals.calories_kcal, unit: 'kcal', color: '#10b981' },
    pro:  { cur: totals.protein_g, goal: goals.protein_g, unit: 'g', color: '#f59e0b' },
    fat:  { cur: totals.fat_g, goal: goals.fat_g, unit: 'g', color: '#f43f5e' },
    carb: { cur: totals.carbs_g, goal: goals.carbs_g, unit: 'g', color: '#8b5cf6' },
    fib:  { cur: totals.fiber_g, goal: goals.fiber_g, unit: 'g', color: '#14b8a6' },
    water:{ cur: totals.fluid_oz, goal: goals.hydration_oz, unit: 'oz', color: '#0ea5e9' },
  } : null);

  $effect(() => {
    initStore();
    loadAll();
    loadCustomFoods();
  });

  function handleKey(e: KeyboardEvent) {
    if (e.key === '/' && document.activeElement?.tagName !== 'INPUT') {
      e.preventDefault();
      (document.getElementById('water-input') as HTMLInputElement)?.focus();
    }
  }
</script>

<svelte:window onkeydown={handleKey} />

<div class="flex h-screen bg-slate-50 overflow-hidden">
  <!-- Sidebar -->
  <div class="w-60 bg-white border-r flex flex-col shrink-0">
    <div class="px-5 pt-6 pb-5 flex items-center gap-3 border-b">
      <div class="flex -space-x-1">
        <div class="w-8 h-8 bg-emerald-600 rounded-2xl flex items-center justify-center ring-2 ring-white"><Apple class="w-4.5 h-4.5 text-white"/></div>
        <div class="w-8 h-8 bg-sky-500 rounded-2xl flex items-center justify-center ring-2 ring-white"><Droplet class="w-4.5 h-4.5 text-white"/></div>
      </div>
      <div>
        <div class="font-semibold text-xl tracking-tighter">NomNom Nanny</div>
        <div class="text-[10px] text-emerald-600 -mt-1">daily nutrition companion</div>
      </div>
    </div>

    <nav class="px-3 py-4 text-sm">
      <button onclick={() => activeTab='today'} class="w-full flex items-center gap-3 px-3 py-2.5 rounded-2xl mb-1 {activeTab==='today' ? 'bg-emerald-50 text-emerald-700 font-medium' : 'hover:bg-slate-100 text-slate-600'}"><Calendar class="w-4 h-4"/> Today</button>
      <button onclick={() => activeTab='history'} class="w-full flex items-center gap-3 px-3 py-2.5 rounded-2xl mb-1 {activeTab==='history' ? 'bg-emerald-50 text-emerald-700 font-medium' : 'hover:bg-slate-100 text-slate-600'}"><History class="w-4 h-4"/> History</button>
      <button onclick={() => activeTab='goals'} class="w-full flex items-center gap-3 px-3 py-2.5 rounded-2xl {activeTab==='goals' ? 'bg-emerald-50 text-emerald-700 font-medium' : 'hover:bg-slate-100 text-slate-600'}"><Target class="w-4 h-4"/> Goals</button>
    </nav>

    <div class="mt-auto p-4 border-t text-[11px] text-slate-400">All data local • Press / for water</div>
  </div>

  <!-- Main -->
  <div class="flex-1 flex flex-col overflow-hidden">
    <div class="h-14 border-b bg-white flex items-center px-6 justify-between shrink-0">
      <div>
        {#if activeTab === 'today'}<DateNavigator date={currentDate} onChange={changeDate} />{/if}
        {#if activeTab === 'history'}<div class="font-semibold text-xl">History (last 14 days)</div>{/if}
        {#if activeTab === 'goals'}<div class="font-semibold text-xl">Your Daily Goals</div>{/if}
      </div>
      <div class="text-xs text-slate-400">{status || (isLoading ? 'Loading…' : '')}</div>
    </div>

    {#if activeTab === 'today'}
      <div class="flex-1 overflow-auto p-6 space-y-6">
        {#if progress && goals}
          <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
            <MacroRing label="Calories" current={progress.kcal.cur} goal={progress.kcal.goal} unit="kcal" color={progress.kcal.color}/>
            <MacroRing label="Protein" current={progress.pro.cur} goal={progress.pro.goal} unit="g" color={progress.pro.color}/>
            <MacroRing label="Fat" current={progress.fat.cur} goal={progress.fat.goal} unit="g" color={progress.fat.color}/>
            <MacroRing label="Carbs" current={progress.carb.cur} goal={progress.carb.goal} unit="g" color={progress.carb.color}/>
            <MacroRing label="Fiber" current={progress.fib.cur} goal={progress.fib.goal} unit="g" color={progress.fib.color}/>
            <MacroRing label="Water" current={progress.water.cur} goal={progress.water.goal} unit="oz" color={progress.water.color} icon={Droplet}/>
          </div>
        {/if}

        <div class="bg-white border rounded-3xl p-5">
          <div class="text-sm font-semibold text-slate-600 mb-3">Quick add</div>
          <div class="flex flex-wrap gap-3">
            <div class="flex items-center bg-sky-50 border border-sky-100 rounded-2xl pl-4 pr-2 py-1.5 gap-2">
              <Droplet class="w-4 h-4 text-sky-500"/>
              <input id="water-input" type="number" bind:value={waterOz} class="w-14 font-mono text-lg bg-transparent border-0 p-0"/>
              <span class="text-sm text-sky-600 pr-1">fl oz water</span>
              <button onclick={logWater} class="px-5 py-2 bg-sky-500 hover:bg-sky-600 text-white text-sm font-medium rounded-2xl flex items-center gap-1"><Plus class="w-3.5 h-3.5"/>Log</button>
            </div>

            <button onclick={() => showCustomForm = !showCustomForm} class="px-5 py-2.5 border rounded-2xl text-sm font-medium flex items-center gap-2"><Plus class="w-4 h-4"/>Manual entry</button>
            <button onclick={openFoodSearch} class="px-5 py-2.5 border border-emerald-200 text-emerald-700 hover:bg-emerald-50 rounded-2xl text-sm font-medium flex items-center gap-2"><Search class="w-4 h-4"/>Search real foods (USDA)</button>
            <button onclick={() => { showMyFoodsModal = true; loadCustomFoods(); }} class="px-5 py-2.5 border border-amber-200 text-amber-700 hover:bg-amber-50 rounded-2xl text-sm font-medium flex items-center gap-2">
              My Foods ({customFoods.length})
            </button>
            <div class="text-[10px] text-slate-400 self-center ml-1">Tip: Use "Search real foods" for accurate data from the USDA database</div>
          </div>

          {#if showCustomForm}
            <div class="mt-4 p-4 bg-slate-50 rounded-2xl space-y-3 text-sm">
              <div class="text-xs text-slate-500">
                Use this for homemade foods or items not in the USDA database. Enter the <strong>total</strong> values for the entire amount you're logging.
              </div>

              <div class="grid grid-cols-1 md:grid-cols-6 gap-3">
                <!-- Name -->
                <div class="md:col-span-6">
                  <div class="text-[10px] text-slate-500 mb-0.5">Food name</div>
                  <input 
                    bind:value={custom.name} 
                    placeholder="e.g. My homemade chicken salad" 
                    class="w-full border rounded-xl px-3 py-2"
                  />
                </div>

                <!-- Nutrients -->
                <div>
                  <div class="text-[10px] text-slate-500 mb-0.5">Total kcal</div>
                  <input type="number" bind:value={custom.kcal} placeholder="e.g. 320" class="w-full border rounded-xl px-3 py-2"/>
                </div>
                <div>
                  <div class="text-[10px] text-slate-500 mb-0.5">Protein (g)</div>
                  <input type="number" bind:value={custom.protein} step="0.1" placeholder="e.g. 28" class="w-full border rounded-xl px-3 py-2"/>
                </div>
                <div>
                  <div class="text-[10px] text-slate-500 mb-0.5">Fat (g)</div>
                  <input type="number" bind:value={custom.fat} step="0.1" placeholder="e.g. 12" class="w-full border rounded-xl px-3 py-2"/>
                </div>
                <div>
                  <div class="text-[10px] text-slate-500 mb-0.5">Carbs (g)</div>
                  <input type="number" bind:value={custom.carbs} step="0.1" placeholder="e.g. 8" class="w-full border rounded-xl px-3 py-2"/>
                </div>
                <div>
                  <div class="text-[10px] text-slate-500 mb-0.5">Fiber (g)</div>
                  <input type="number" bind:value={custom.fiber} step="0.1" placeholder="e.g. 3" class="w-full border rounded-xl px-3 py-2"/>
                </div>

                <!-- Actions -->
                <div class="md:col-span-6 flex gap-2 justify-end pt-1">
                  <button onclick={() => showCustomForm=false} class="px-4 py-2 text-slate-600 hover:text-slate-800">Cancel</button>
                  <button onclick={logCustomEntry} class="px-6 py-2 bg-emerald-600 hover:bg-emerald-700 text-white rounded-xl font-medium">Add to log</button>
                  <button onclick={saveCurrentAsCustomFood} class="px-4 py-2 border border-amber-300 text-amber-700 hover:bg-amber-50 rounded-xl text-sm">
                    Save as Custom Food
                  </button>
                </div>
              </div>
            </div>
          {/if}
        </div>

        <div class="bg-white border rounded-3xl overflow-hidden">
          <div class="px-5 py-3 border-b bg-slate-50/60 font-semibold flex justify-between">
            <div>Today's log ({entries.length})</div>
            {#if averages7}<div class="text-xs text-slate-500">7-day avg: {averages7.calories_kcal.toFixed(0)} kcal</div>{/if}
          </div>
          {#if entries.length === 0}
            <div class="p-8 text-center text-slate-400 text-sm">No entries yet — try the quick add buttons above.</div>
          {:else}
            <div class="divide-y text-sm">
              {#each entries as e (e.id)}
                <div class="px-5 py-3 flex justify-between group hover:bg-slate-50">
                  <div><span class="font-medium">{e.description}</span> <span class="text-slate-400">({e.amount} {e.unit})</span></div>
                  <div class="flex items-center gap-4 text-xs tabular-nums">
                    <span class="text-emerald-600">{e.calories_kcal.toFixed(0)} kcal</span>
                    <span class="text-sky-600">{e.fluid_oz.toFixed(1)} oz</span>
                    <button onclick={() => deleteEntry(e.id)} class="opacity-0 group-hover:opacity-100 text-rose-500"><Trash2 class="w-4 h-4"/></button>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    {/if}

    {#if activeTab === 'history'}
      <div class="p-6 overflow-auto">
        <div class="bg-white border rounded-3xl overflow-hidden">
          <table class="w-full text-sm">
            <thead class="bg-slate-50 text-slate-500"><tr><th class="text-left px-5 py-3">Date</th><th>Calories</th><th>Protein</th><th>Water</th><th>Items</th></tr></thead>
            <tbody class="divide-y">
              {#each history as d}<tr class="hover:bg-emerald-50 cursor-pointer" onclick={()=>{currentDate=d.log_date; activeTab='today';}}>
                <td class="px-5 py-3 font-medium">{d.log_date}</td>
                <td class="text-center tabular-nums">{d.calories_kcal.toFixed(0)}</td>
                <td class="text-center tabular-nums">{d.protein_g.toFixed(1)}g</td>
                <td class="text-center text-sky-600 tabular-nums">{d.fluid_oz.toFixed(1)}oz</td>
                <td class="text-center text-slate-400">{d.entry_count}</td>
              </tr>{/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}

    {#if activeTab === 'goals'}
      <div class="p-6 max-w-2xl space-y-6">
        <!-- Daily Goals -->
        <div class="bg-white border rounded-3xl p-6">
          <div class="font-semibold mb-4">Edit daily targets</div>
          {#if goals}
            <div class="grid grid-cols-2 gap-4">
              {#each [['Calories (kcal)', 'calories_kcal'], ['Protein (g)', 'protein_g'], ['Fat (g)', 'fat_g'], ['Carbs (g)', 'carbs_g'], ['Fiber (g)', 'fiber_g'], ['Water (fl oz)', 'hydration_oz']] as [label, key]}
                <label class="block"><span class="text-xs text-slate-500">{label}</span>
                  <input type="number" bind:value={goals[key as keyof Goal]} class="mt-1 w-full border rounded-2xl px-4 py-2.5 text-lg"/>
                </label>
              {/each}
            </div>
            <button onclick={async ()=>{await invoke('update_goals_cmd',{goal:goals}); await loadAll(); status='Saved';}} class="mt-5 w-full py-3 bg-emerald-600 text-white rounded-2xl font-medium">Save Goals</button>
          {/if}
        </div>

        <!-- USDA API Key -->
        <div class="bg-white border rounded-3xl p-6">
          <div class="font-semibold mb-2">USDA FoodData Central API Key</div>
          <p class="text-xs text-slate-500 mb-3">
            Required for the "Search real foods" feature. Get a free key at 
            <a href="https://fdc.nal.usda.gov/api-key-signup" target="_blank" class="text-emerald-600 underline">fdc.nal.usda.gov/api-key-signup</a>
          </p>

          <div class="flex gap-2">
            <input 
              type="password" 
              bind:value={apiKey} 
              placeholder="Paste your USDA API key here"
              class="flex-1 border rounded-2xl px-4 py-2.5 text-sm font-mono"
              onblur={saveApiKey}
            />
            <button 
              onclick={saveApiKey}
              class="px-6 py-2.5 bg-emerald-600 hover:bg-emerald-700 text-white rounded-2xl text-sm font-medium"
            >
              Save
            </button>
          </div>

          <div class="text-[10px] text-slate-400 mt-2">
            Your key is stored securely on your device.
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<!-- Food Search Modal -->
<Modal bind:open={showFoodModal} onClose={closeFoodModal} title="Search USDA foods">
  <div>
    <input 
      id="food-search-input" 
      bind:value={foodQuery} 
      oninput={onSearchInput} 
      onkeydown={onSearchKeydown}
      placeholder="Search foods (e.g. chicken, banana, oats)..." 
      class="w-full border rounded-2xl px-4 py-3 text-lg mb-4"
    />
    {#if !selectedFood}
      {#if isSearching}
        <div class="text-sm py-2 text-slate-500">Searching USDA database…</div>
      {:else if detailLoading}
        <div class="text-sm py-2 text-slate-500">Loading food details…</div>
      {:else if searchError}
        <div class="text-sm p-3 bg-red-50 border border-red-200 text-red-700 rounded-2xl">
          {searchError}
        </div>
      {:else if searchResults.length > 0}
        <div class="text-xs text-slate-500 mb-1 px-1">Click a food to see nutrition info and choose quantity</div>
        <div class="max-h-80 overflow-auto border rounded-2xl divide-y">
          {#each searchResults as h}
            <button onclick={() => selectFood(h)} class="w-full text-left px-4 py-3 hover:bg-emerald-50 text-sm flex justify-between">
              <span>{h.description}</span>
              <span class="text-xs text-slate-400 self-center">{h.data_type}</span>
            </button>
          {/each}
        </div>
      {:else if foodQuery.trim().length > 1}
        <div class="text-sm text-slate-500 py-2">No results found. Try a different term or check your API key in the Goals tab.</div>
      {/if}
    {:else}
      <div class="border rounded-2xl p-4 bg-slate-50 space-y-4">
        <div class="font-medium">{selectedFood.description}</div>

        <div>
          <div class="text-xs text-slate-500 mb-1">Choose base serving</div>
          {#each selectedFood.portions as p, i}
            <button 
              onclick={()=>{selectedPortionIndex=i; portionMultiplier=1;}} 
              class="block w-full text-left px-3 py-1.5 my-0.5 rounded-xl text-sm {selectedPortionIndex===i ? 'bg-emerald-100 border border-emerald-300' : 'hover:bg-white border border-transparent'}">
              {p.label}
            </button>
          {/each}
        </div>

        <div>
          <div class="text-xs text-slate-500 mb-1">Quantity multiplier (or enter exact grams below)</div>
          <div class="flex items-center gap-3">
            <input type="range" min="0.25" max="4" step="0.05" bind:value={portionMultiplier} class="flex-1 accent-emerald-600"/>
            <input 
              type="number" 
              bind:value={portionMultiplier} 
              step="0.05" 
              class="w-24 border rounded-xl px-3 py-1.5 text-sm font-mono"
            />
          </div>
          <div class="text-[10px] text-slate-400 mt-1">
            This multiplies the selected serving size.
          </div>
        </div>

        {#if previewLoading}
          <div class="p-3 bg-white border rounded-2xl text-sm text-slate-500 text-center">
            Calculating...
          </div>
        {:else if previewNutrients}
          <div class="p-3 bg-white border rounded-2xl">
            <div class="text-xs text-slate-500 mb-1">Calculated for this quantity:</div>
            <div class="grid grid-cols-5 gap-2 text-center text-sm">
              <div><div class="font-semibold text-emerald-600">{previewNutrients.kcal.toFixed(0)}</div><div class="text-[10px]">kcal</div></div>
              <div><div class="font-semibold">{previewNutrients.protein.toFixed(1)}</div><div class="text-[10px]">protein</div></div>
              <div><div class="font-semibold">{previewNutrients.fat.toFixed(1)}</div><div class="text-[10px]">fat</div></div>
              <div><div class="font-semibold">{previewNutrients.carbs.toFixed(1)}</div><div class="text-[10px]">carbs</div></div>
              <div><div class="font-semibold">{previewNutrients.fiber.toFixed(1)}</div><div class="text-[10px]">fiber</div></div>
            </div>
          </div>
        {/if}
      </div>

      <button 
        onclick={logSelectedFood} 
        disabled={logging}
        class="mt-4 w-full py-3 bg-emerald-600 hover:bg-emerald-700 disabled:bg-emerald-400 text-white rounded-2xl font-medium flex items-center justify-center gap-2"
      >
        {logging ? 'Logging...' : 'Log this amount'}
      </button>
    {/if}
  </div>
</Modal>

<!-- My Custom Foods Modal -->
<Modal bind:open={showMyFoodsModal} onClose={() => showMyFoodsModal = false} title="My Custom Foods">
  <div class="space-y-4 max-h-[70vh] overflow-auto">
    {#if customFoods.length === 0}
      <p class="text-slate-500 text-sm">You haven't saved any custom foods yet. Use "Manual entry" and then save it from there.</p>
    {:else}
      {#each customFoods as food (food.id)}
        <div class="border rounded-2xl p-3">
          <!-- Header -->
          <div class="flex justify-between items-start mb-2">
            <input 
              bind:value={food.name} 
              class="font-medium border-b border-transparent focus:border-emerald-300 px-1 py-0.5 w-48"
              onblur={() => updateCustomFood(food)}
            />
            <div class="flex gap-2">
              <button onclick={() => startLoggingFood(food)} 
                      class="text-xs px-3 py-1 bg-emerald-600 text-white rounded hover:bg-emerald-700">
                Log this food
              </button>
              <button onclick={() => deleteCustomFood(food.id)} class="text-xs px-2 py-1 text-rose-600 hover:bg-rose-50 rounded">Delete</button>
            </div>
          </div>

          <!-- Editable base per-100g values -->
          <div class="grid grid-cols-5 gap-2 text-xs mb-3">
            <div>
              <div class="text-[10px] text-slate-500">kcal /100g</div>
              <input type="number" bind:value={food.kcal_per_100g} step="0.1" 
                     class="w-full border rounded px-1 py-0.5 font-mono text-sm"
                     onblur={() => updateCustomFood(food)} />
            </div>
            <div>
              <div class="text-[10px] text-slate-500">Protein</div>
              <input type="number" bind:value={food.protein_per_100g} step="0.1" 
                     class="w-full border rounded px-1 py-0.5 font-mono text-sm"
                     onblur={() => updateCustomFood(food)} />
            </div>
            <div>
              <div class="text-[10px] text-slate-500">Fat</div>
              <input type="number" bind:value={food.fat_per_100g} step="0.1" 
                     class="w-full border rounded px-1 py-0.5 font-mono text-sm"
                     onblur={() => updateCustomFood(food)} />
            </div>
            <div>
              <div class="text-[10px] text-slate-500">Carbs</div>
              <input type="number" bind:value={food.carbs_per_100g} step="0.1" 
                     class="w-full border rounded px-1 py-0.5 font-mono text-sm"
                     onblur={() => updateCustomFood(food)} />
            </div>
            <div>
              <div class="text-[10px] text-slate-500">Fiber</div>
              <input type="number" bind:value={food.fiber_per_100g} step="0.1" 
                     class="w-full border rounded px-1 py-0.5 font-mono text-sm"
                     onblur={() => updateCustomFood(food)} />
            </div>
          </div>

          <!-- Advanced logging UI -->
          {#if activeLogFoodId === food.id}
            <div class="mt-3 p-3 bg-amber-50 border border-amber-200 rounded-xl">
              <div class="text-sm font-medium mb-2">Log {food.name}</div>

              <div class="flex gap-3 items-end mb-3">
                <div>
                  <div class="text-[10px] text-slate-500">Grams</div>
                  <input type="number" bind:value={logGrams} step="1" class="w-24 border rounded px-2 py-1 text-sm" />
                </div>
                <button onclick={() => confirmLogFromMyFoods(food)} 
                        class="px-4 py-1.5 bg-emerald-600 text-white text-sm rounded-xl">
                  Log this
                </button>
                <button onclick={() => activeLogFoodId = null} class="px-3 py-1.5 text-sm">Cancel</button>
              </div>

              <div class="text-xs mb-2">
                <label class="flex items-center gap-2 mb-1">
                  <input type="checkbox" bind:checked={useNutrientOverride} />
                  <span>Override nutrients for this entry only</span>
                </label>

                {#if !useNutrientOverride}
                  <div class="text-slate-500">Using saved per-100g values × {logGrams}g</div>
                {:else}
                  <div class="grid grid-cols-5 gap-1 text-[10px] mt-1">
                    <div><span class="text-slate-500">kcal</span><br>
                      <input type="number" bind:value={overrideNutrients.kcal} step="0.1" class="w-full border text-xs px-1 py-0.5 rounded" /></div>
                    <div><span class="text-slate-500">protein</span><br>
                      <input type="number" bind:value={overrideNutrients.protein} step="0.1" class="w-full border text-xs px-1 py-0.5 rounded" /></div>
                    <div><span class="text-slate-500">fat</span><br>
                      <input type="number" bind:value={overrideNutrients.fat} step="0.1" class="w-full border text-xs px-1 py-0.5 rounded" /></div>
                    <div><span class="text-slate-500">carbs</span><br>
                      <input type="number" bind:value={overrideNutrients.carbs} step="0.1" class="w-full border text-xs px-1 py-0.5 rounded" /></div>
                    <div><span class="text-slate-500">fiber</span><br>
                      <input type="number" bind:value={overrideNutrients.fiber} step="0.1" class="w-full border text-xs px-1 py-0.5 rounded" /></div>
                  </div>
                {/if}
              </div>
            </div>
          {:else}
            <div class="flex gap-2">
              <button onclick={() => startLoggingFood(food)} 
                      class="text-xs px-3 py-1 bg-amber-600 text-white rounded hover:bg-amber-700">
                Log with custom quantity + overrides
              </button>
              <button onclick={() => logFromCustomFood(food, 100)} 
                      class="text-xs px-3 py-1 bg-emerald-100 text-emerald-700 rounded hover:bg-emerald-200">
                Quick log 100g
              </button>
            </div>
          {/if}
        </div>
      {/each}
    {/if}

    <div class="pt-2 border-t">
      <button onclick={() => {
        saveCurrentAsCustomFood();
      }} class="text-sm px-4 py-2 border border-emerald-300 text-emerald-700 hover:bg-emerald-50 rounded-2xl">
        Save current manual entry as new Custom Food
      </button>
    </div>

    <!-- Small dialog for saving with grams info -->
    {#if showSaveCustomDialog}
      <div class="mt-4 p-4 border border-emerald-200 bg-emerald-50 rounded-2xl">
        <div class="text-sm font-medium mb-2">Save as Custom Food</div>
        <div class="text-xs text-slate-600 mb-3">
          You entered totals for a certain amount. How many grams was that entry?
        </div>
        <div class="flex gap-2 items-end">
          <div>
            <div class="text-[10px] text-slate-500">Grams this entry represented</div>
            <input 
              type="number" 
              bind:value={saveCustomGrams} 
              class="w-28 border rounded-xl px-3 py-1.5 text-sm" 
            />
          </div>
          <button onclick={confirmSaveCurrentAsCustomFood} class="px-4 py-2 bg-emerald-600 text-white rounded-xl text-sm">Save</button>
          <button onclick={() => showSaveCustomDialog = false} class="px-4 py-2 text-slate-600">Cancel</button>
        </div>
        <div class="text-[10px] text-slate-500 mt-2">
          The app will convert your totals into per-100g values for future scaling.
        </div>
      </div>
    {/if}
  </div>
</Modal>
