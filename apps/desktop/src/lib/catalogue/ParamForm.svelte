<script lang="ts">
  /**
   * ParamForm — auto-renders input controls for a list of EndpointParams.
   *
   * Required params are shown directly. Optional params are nested under an
   * "Advanced filters" disclosure toggle. Serializes values into the bound
   * `values` record, so callers can pass them directly to the API.
   *
   * param_type dispatch:
   *   "String" with name containing "right" → radio Calls / Puts / Both
   *   "Date" or name ends with "date"        → date picker (YYYYMMDD wire)
   *   "Integer" | "Number"                   → number input
   *   else                                   → text input
   */
  import { ChevronDown, ChevronRight } from "lucide-svelte";
  import { renderMarkdown } from "$lib/util/md";
  import type { EndpointParam } from "$lib/api";

  let {
    params,
    values = $bindable<Record<string, string>>({}),
    /** Param names managed externally by SmartFilters — excluded here. */
    excludeNames = [],
  }: {
    params: EndpointParam[];
    values: Record<string, string>;
    excludeNames?: string[];
  } = $props();

  // Split into required vs optional, excluding SmartFilters-owned params
  // and common structural params (symbol, root) that callers always supply.
  const STRUCTURAL = new Set(["root", "symbol"]);

  const visibleParams = $derived(
    params.filter((p) => !excludeNames.includes(p.name) && !STRUCTURAL.has(p.name))
  );

  const requiredParams = $derived(visibleParams.filter((p) => p.required));
  const optionalParams = $derived(visibleParams.filter((p) => !p.required));

  let advancedOpen = $state(false);

  // Helpers
  function inputType(p: EndpointParam): "right-radio" | "date" | "number" | "text" {
    const n = p.name.toLowerCase();
    const t = p.param_type.toLowerCase();
    if (n === "right" || (t.includes("string") && n.includes("right"))) return "right-radio";
    if (t === "date" || n.endsWith("date") || n.endsWith("_date")) return "date";
    if (t === "integer" || t === "number" || t === "i32" || t === "i64" || t === "f64") return "number";
    return "text";
  }

  function placeholder(p: EndpointParam): string {
    const n = p.name.toLowerCase();
    if (n.endsWith("_filter") || n.includes("filter")) return "* (all)";
    if (n === "expiration") return "* (all) or YYYYMMDD";
    if (n === "strike") return "* (all) or price";
    if (n.endsWith("date")) return "YYYY-MM-DD";
    return "";
  }

  /** Convert display date (YYYY-MM-DD) to wire format (YYYYMMDD) */
  function dateToWire(display: string): string {
    return display.replace(/-/g, "");
  }

  /** Convert wire date (YYYYMMDD) to display format (YYYY-MM-DD) */
  function wireToDate(wire: string): string {
    if (!wire || wire.length !== 8) return wire;
    return `${wire.slice(0, 4)}-${wire.slice(4, 6)}-${wire.slice(6, 8)}`;
  }

  function getDateDisplay(name: string): string {
    const wire = values[name] ?? "";
    return wireToDate(wire);
  }

  function setDateValue(name: string, display: string) {
    values = { ...values, [name]: dateToWire(display) };
  }

  function setValue(name: string, val: string) {
    values = { ...values, [name]: val };
  }
</script>

{#if visibleParams.length === 0}
  <!-- Nothing to show — SmartFilters handles everything for this endpoint -->
{:else}
  <div class="param-form">
    <!-- Required params -->
    {#if requiredParams.length > 0}
      <div class="param-group">
        {#each requiredParams as p (p.name)}
          <div class="param-row">
            <label class="param-label" for="pf-{p.name}">
              {p.name.replace(/_/g, " ")}
              <span class="required-star" aria-label="required">*</span>
            </label>
            {#if p.description}
              <p class="param-hint markdown-inline">{@html renderMarkdown(p.description)}</p>
            {/if}

            {#if inputType(p) === "right-radio"}
              <div class="tile-picker" role="radiogroup" aria-label="Option right">
                {#each [
                  { id: "both", label: "Both" },
                  { id: "C",    label: "Calls" },
                  { id: "P",    label: "Puts" },
                ] as opt (opt.id)}
                  <button
                    type="button"
                    role="radio"
                    aria-checked={(values[p.name] ?? "both") === opt.id}
                    class="tile-btn"
                    class:active={(values[p.name] ?? "both") === opt.id}
                    onclick={() => setValue(p.name, opt.id)}
                  >{opt.label}</button>
                {/each}
              </div>

            {:else if inputType(p) === "date"}
              <input
                id="pf-{p.name}"
                type="date"
                class="field-input"
                value={getDateDisplay(p.name)}
                oninput={(e) => setDateValue(p.name, (e.currentTarget as HTMLInputElement).value)}
                aria-required="true"
              />

            {:else if inputType(p) === "number"}
              <input
                id="pf-{p.name}"
                type="number"
                class="field-input"
                value={values[p.name] ?? ""}
                placeholder={placeholder(p)}
                oninput={(e) => setValue(p.name, (e.currentTarget as HTMLInputElement).value)}
                aria-required="true"
              />

            {:else}
              <input
                id="pf-{p.name}"
                type="text"
                class="field-input"
                value={values[p.name] ?? ""}
                placeholder={placeholder(p)}
                oninput={(e) => setValue(p.name, (e.currentTarget as HTMLInputElement).value)}
                aria-required="true"
              />
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    <!-- Optional / advanced params -->
    {#if optionalParams.length > 0}
      <div class="advanced-section">
        <button
          type="button"
          class="advanced-toggle"
          aria-expanded={advancedOpen}
          onclick={() => (advancedOpen = !advancedOpen)}
        >
          {#if advancedOpen}
            <ChevronDown size={13} strokeWidth={1.75} />
          {:else}
            <ChevronRight size={13} strokeWidth={1.75} />
          {/if}
          Advanced filters
          <span class="adv-count text-caption">({optionalParams.length})</span>
          {#if Object.keys(values).some((k) => optionalParams.some((p) => p.name === k && values[k]))}
            <span class="active-badge">active</span>
          {/if}
        </button>

        {#if advancedOpen}
          <div class="param-group advanced-group">
            {#each optionalParams as p (p.name)}
              <div class="param-row">
                <label class="param-label" for="pfopt-{p.name}">
                  {p.name.replace(/_/g, " ")}
                </label>
                {#if p.description}
                  <p class="param-hint markdown-inline">{@html renderMarkdown(p.description)}</p>
                {/if}

                {#if inputType(p) === "right-radio"}
                  <div class="tile-picker" role="radiogroup" aria-label="Option right">
                    {#each [
                      { id: "both", label: "Both" },
                      { id: "C",    label: "Calls" },
                      { id: "P",    label: "Puts" },
                    ] as opt (opt.id)}
                      <button
                        type="button"
                        role="radio"
                        aria-checked={(values[p.name] ?? "both") === opt.id}
                        class="tile-btn"
                        class:active={(values[p.name] ?? "both") === opt.id}
                        onclick={() => setValue(p.name, opt.id)}
                      >{opt.label}</button>
                    {/each}
                  </div>

                {:else if inputType(p) === "date"}
                  <input
                    id="pfopt-{p.name}"
                    type="date"
                    class="field-input"
                    value={getDateDisplay(p.name)}
                    oninput={(e) => setDateValue(p.name, (e.currentTarget as HTMLInputElement).value)}
                  />

                {:else if inputType(p) === "number"}
                  <input
                    id="pfopt-{p.name}"
                    type="number"
                    class="field-input"
                    value={values[p.name] ?? ""}
                    placeholder={placeholder(p)}
                    oninput={(e) => setValue(p.name, (e.currentTarget as HTMLInputElement).value)}
                  />

                {:else}
                  <input
                    id="pfopt-{p.name}"
                    type="text"
                    class="field-input"
                    value={values[p.name] ?? ""}
                    placeholder={placeholder(p)}
                    oninput={(e) => setValue(p.name, (e.currentTarget as HTMLInputElement).value)}
                  />
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .param-form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .param-group {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .advanced-group {
    padding: var(--sp-4);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    margin-top: var(--sp-2);
  }

  .param-row {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .param-label {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    color: var(--fg);
    text-transform: capitalize;
    display: flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .required-star {
    color: var(--bad);
    font-size: var(--text-body-sm);
    line-height: 1;
  }

  .param-hint {
    font-size: var(--text-caption);
    color: var(--fg-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
    line-height: 1.4;
    margin: 0;
  }

  .field-input {
    height: 32px;
    padding: 0 var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    font-size: var(--text-body-sm);
    font-family: var(--font-ui);
    outline: none;
    width: 100%;
    max-width: 360px;
    transition:
      border-color var(--dur-fast) var(--ease-standard),
      box-shadow var(--dur-fast) var(--ease-standard);
  }

  .field-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }

  .field-input::placeholder {
    color: var(--fg-subtle);
  }

  /* Right tile picker */
  .tile-picker {
    display: flex;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  .tile-btn {
    padding: var(--sp-2) var(--sp-4);
    background: var(--surface-2);
    border: 1.5px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
    outline: none;
  }

  .tile-btn:hover:not(.active) {
    background: var(--surface-3);
    border-color: var(--border-strong);
    color: var(--fg);
  }

  .tile-btn.active {
    background: var(--accent-tint);
    border-color: var(--accent);
    color: var(--accent-hi);
  }

  .tile-btn:focus-visible { box-shadow: var(--shadow-glow-accent); }

  /* Advanced toggle */
  .advanced-section {
    display: flex;
    flex-direction: column;
  }

  .advanced-toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 0;
    background: transparent;
    border: none;
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease-standard);
    outline: none;
    text-align: left;
  }

  .advanced-toggle:hover { color: var(--fg); }
  .advanced-toggle:focus-visible { box-shadow: var(--shadow-glow-accent); border-radius: var(--r-sm); }

  .adv-count {
    color: var(--fg-subtle);
    font-weight: var(--weight-normal);
    text-transform: none;
    letter-spacing: 0;
  }

  .active-badge {
    padding: 1px 6px;
    border-radius: var(--r-pill);
    background: var(--accent-tint);
    color: var(--accent-hi);
    font-size: var(--text-caption);
    font-weight: var(--weight-semi);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
