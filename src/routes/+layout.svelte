<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';

  let { children } = $props();
</script>

<svelte:head>
  <title>NomNom Nanny</title>
</svelte:head>

{@render children?.()}
