<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { initTheme } from "$lib/stores/app.svelte";

  let { children } = $props();
  // Apply theme as early as possible so the gate doesn't flash dark on light.
  onMount(initTheme);
</script>

{@render children()}
