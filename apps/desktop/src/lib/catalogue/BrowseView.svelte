<script lang="ts">
  /**
   * BrowseView — guided composition page, YAML-catalogue driven.
   *
   * Six focused steps:
   *   1. Asset class (AssetClassPicker)
   *   2. Symbols (UniverseSelector)
   *   3. Data kind (KindGrid — catalogue cards grouped by tag)
   *   4. Time range (RangePicker)
   *   5. Advanced filters (SmartFilters + ParamForm — auto-rendered from params)
   *   6. Output format
   *
   * Sticky footer summarises selection and exposes Queue button.
   */

  import { Loader2, Check, ArrowUpRight, Bookmark, Plus, Play } from "lucide-svelte";
  import AssetClassPicker from "$lib/catalogue/AssetClassPicker.svelte";
  import FlatfilesShelf from "$lib/runners/FlatfilesShelf.svelte";
  import IndexPresetsShelf from "$lib/runners/IndexPresetsShelf.svelte";
  import UniverseSelector from "$lib/catalogue/UniverseSelector.svelte";
  import KindGrid from "$lib/catalogue/KindGrid.svelte";
  import RangePicker from "$lib/catalogue/RangePicker.svelte";
  import IntervalPicker from "$lib/catalogue/IntervalPicker.svelte";
  import SmartFilters from "$lib/catalogue/SmartFilters.svelte";
  import ParamForm from "$lib/catalogue/ParamForm.svelte";
  import { api, type EnqueueArgs, type EnqueuePlan } from "$lib/api";
  import { app, tierForKind, log, type AssetClass, refreshQueueSnapshot, openEndpointRunner} from "$lib/stores/app.svelte";
  import { saveSearch } from "$lib/persistence/savedSearches";
  import { openUrl } from "@tauri-apps/plugin-opener";

  // ── Step state ────────────────────────────────────────────────
  let assetClass = $state<AssetClass>("stock");
  let symbols    = $state<string[]>([]);
  let kindId     = $state("");
  let start      = $state("");
  let end        = $state("");
  let interval   = $state("tick");
  let format     = $state<"parquet" | "csv" | "jsonl" | "json">("parquet");

  // Shared param values map — written to by SmartFilters and ParamForm
  let paramValues = $state<Record<string, string>>({});

  /** Apply a hand-over from another view (Library's "Download more
   *  dates", a dataset detail page) exactly once, then clear it so
   *  navigating back to Browse later does not re-apply a stale
   *  selection. */
  $effect(() => {
    const intent = app.browseIntent;
    if (!intent) return;
    app.browseIntent = null;
    assetClass = assetClassOf(intent.kind);
    kindId = intent.kind;
    if (intent.symbol) symbols = [intent.symbol];
    paramValues = {};
  });

  function assetClassOf(kind: string): AssetClass {
    if (kind.startsWith("option_")) return "option";
    if (kind.startsWith("index_")) return "index";
    if (kind.startsWith("rate_") || kind.startsWith("interest_")) return "rate";
    return "stock";
  }

  // ── Queue state ───────────────────────────────────────────────
  type QueueStatus = "idle" | "queuing" | "done" | "error";
  let queueStatus = $state<QueueStatus>("idle");
  let queueMsg    = $state("");
  let lastQueuedCount = $state(0);

  // ── Selected catalogue entry ──────────────────────────────────
  const selectedEntry = $derived(
    app.catalogue.find((e) => e.name === kindId) ?? null
  );

  const selectedParams = $derived(selectedEntry?.params ?? []);

  // Params owned by SmartFilters or higher-level steps (excluded
  // from ParamForm's raw input list). Step 4 RangePicker owns
  // start_date / end_date; Step 5 IntervalPicker owns interval /
  // start_time / end_time; SmartFilters owns the option filters.
  const SMART_FILTER_PARAMS = new Set([
    "right", "max_dte", "min_dte",
    "strike_filter_low", "strike_filter_high", "strike",
    "expiration",
    // owned by Step 4 RangePicker — surfacing them again as raw
    // inputs creates two competing date sources for the same range.
    "start_date", "end_date",
    // Single-`date` endpoints fan out into N daily tasks via the
    // backend's `trading_days` enumerator. Step 4's range picker is
    // the source of truth; this raw input is dead UI for them.
    "date",
    // owned by Step 5 IntervalPicker (when granularity applies).
    "start_time", "end_time", "interval",
    // always derived from the universe selector — never expose raw.
    "root", "symbol", "request_type",
  ]);

  // Reset param values when kind changes
  $effect(() => {
    if (kindId) paramValues = {};
  });

  // ── Step visibility ───────────────────────────────────────────
  // Each step renders only when the selected endpoint actually
  // consumes the corresponding params. Endpoints like
  // `stock_list_symbols` have no date / time params at all — showing
  // an empty RangePicker would mislead users into thinking they need
  // to pick a range that the request will silently ignore.
  //
  // Two flavours of "needs Step 4":
  //   1. Range endpoints (`start_date` + `end_date`) — server consumes
  //      both directly and emits one row per trading day.
  //   2. Point endpoints (`date` only, e.g. `*_at_time_*`) — server
  //      takes a single day per call. The Tauri queue command fans
  //      these out via `trading_days(symbol, start, end)`, so picking
  //      a range here transparently queues N tasks.
  //
  // Either case, Step 4 owns the date axis. ParamForm suppresses the
  // raw `date` input so we don't get two competing date sources.
  const hasRange = $derived(
    selectedParams.some((p) => p.name === "start_date" || p.name === "end_date")
  );
  const hasPointDate = $derived(
    !hasRange && selectedParams.some((p) => p.name === "date")
  );
  const showRange = $derived(hasRange || hasPointDate);
  /** Some endpoints answer a question ("which expirations exist for
   *  SPX?") instead of producing a per-day dataset — the nine `list`
   *  endpoints and the calendar lookups. They declare no date axis at
   *  all, so the queue has nothing to fan out over and `enqueue`
   *  rejects them with "pass date or start+end". Browse used to let the
   *  user reach a fully-enabled Queue button for these and then fail
   *  every time; they run one-shot instead.
   *
   *  Keyed on the absence of a date parameter rather than on a
   *  subcategory name, so an endpoint that grows one stops being
   *  run-once without an edit here. */
  const isRunOnce = $derived(!!kindId && !showRange);
  /** `stock_list_symbols` and `option_list_symbols` take no arguments
   *  at all — requiring a symbol for them would block the only thing
   *  they do. */
  const needsSymbol = $derived(
    selectedParams.some((p) => p.name === "symbol" || p.name === "root"),
  );
  const showInterval = $derived(
    selectedParams.some((p) =>
      p.name === "start_time" || p.name === "end_time" || p.name === "interval"
    )
  );
  /** Params surfaced in Step "Filters" — drops everything owned by an
   *  earlier step. If nothing's left we hide the filters step entirely. */
  const filterParams = $derived(
    selectedParams.filter((p) => !SMART_FILTER_PARAMS.has(p.name))
  );
  const showFilters = $derived(filterParams.length > 0);

  /** Dynamic step number for sections after the fixed 1-2-3. Each
   *  optional step that ships pushes the output step's index up by 1.
   *  Keeps the numbering tight when an endpoint skips dates / interval. */
  function stepIndex(after: "range" | "interval" | "filters" | "output"): number {
    let n = 3;
    if (showRange)    { n++; if (after === "range") return n; }
    if (showInterval) { n++; if (after === "interval") return n; }
    if (showFilters)  { n++; if (after === "filters") return n; }
    if (after === "output") return n + 1;
    return n;
  }

  // ── Tier verdict for selected kind ───────────────────────────
  const verdict = $derived(tierForKind(kindId));
  const gated   = $derived(!!app.tierStatus && !verdict.allowed);

  // Required tier label for the upgrade button
  const upgradeRequiredTier = $derived(verdict.required);

  // ── Summary label for the sticky footer ──────────────────────
  const summaryKindTitle = $derived(
    selectedEntry?.summary || kindId || "—"
  );

  const summaryRange = $derived.by(() => {
    if (!start && !end) return "no range";
    if (start && end) {
      const sy = start.slice(0, 4);
      const ey = end.slice(0, 4);
      return sy === ey ? start.slice(0, 7) + "…" + end.slice(5) : `${sy}–${ey}`;
    }
    return start || end;
  });

  const readyToQueue = $derived(
    symbols.length > 0 && !!kindId && !gated && !isRunOnce && (!showRange || (!!start && !!end))
  );
  /** The run-once equivalent: one symbol is all a list endpoint needs,
   *  and the argument-free ones need not even that. */
  const readyToRun = $derived(
    !!kindId && !gated && isRunOnce && (!needsSymbol || symbols.length > 0),
  );

  function runOnce() {
    const args: Record<string, string> = { ...extraArgs() };
    for (const p of selectedParams) {
      if (p.name === "symbol" || p.name === "root") {
        if (symbols[0]) args[p.name] = symbols[0];
      }
      else if (p.name === "expiration" && paramValues["expiration"]) {
        args[p.name] = paramValues["expiration"];
      } else if (paramValues[p.name]) {
        args[p.name] = String(paramValues[p.name]);
      }
    }
    openEndpointRunner(kindId, args, format);
  }

  /** Estimated number of queue tasks the current selection will fan
   *  out into. Range endpoints (`start_date`+`end_date`) ship one
   *  call covering the whole window — one task per symbol. Point-date
   *  endpoints (`date` only, e.g. `*_at_time_*`) are fanned out by
   *  the backend's `trading_days()` enumerator — one task per
   *  (symbol, trading day). 252/365 is the US-equity calendar
   *  approximation; the backend computes the exact count at enqueue
   *  time. Shown so a 500-symbol × 5-year run doesn't ambush the user. */
  const taskEstimate = $derived.by<number>(() => {
    const syms = symbols.length;
    if (syms === 0 || !kindId) return 0;
    if (!showRange) return syms;
    if (!start || !end) return 0;
    if (!hasPointDate) return syms;
    const s = new Date(`${start.slice(0, 4)}-${start.slice(4, 6)}-${start.slice(6, 8)}`);
    const e = new Date(`${end.slice(0, 4)}-${end.slice(4, 6)}-${end.slice(6, 8)}`);
    const calendarDays = Math.max(1, Math.round((e.getTime() - s.getTime()) / 86_400_000) + 1);
    const trading = Math.max(1, Math.round(calendarDays * 252 / 365));
    return syms * trading;
  });

  // The exact plan, from the code that does the queueing.
  //
  // The approximation above cannot know the things the app now hides:
  // that a window wider than a year becomes several requests, or that
  // the endpoints refusing `expiration=*` fan out over every live
  // expiration — which for a liquid name is hundreds. Someone selecting
  // greeks over three years is committing to a lot more than the
  // trading-day count suggests, and used to find that out from the
  // queue filling up.
  let plan = $state<EnqueuePlan | null>(null);
  let planning = $state(false);

  $effect(() => {
    // Track what the plan depends on.
    const deps = [kindId, symbols[0], start, end, format, paramValues["expiration"]];
    void deps;

    if (!readyToQueue || !symbols[0]) {
      plan = null;
      return;
    }
    let cancelled = false;
    planning = true;
    // The plan can cost a round trip (trading days, expirations), so
    // settle before asking rather than on every keystroke.
    const timer = setTimeout(() => {
      api
        .estimate({ ...enqueueArgsFor(symbols[0]) })
        .then((p) => {
          if (!cancelled) plan = p;
        })
        .catch(() => {
          if (!cancelled) plan = null;
        })
        .finally(() => {
          if (!cancelled) planning = false;
        });
    }, 400);
    return () => {
      cancelled = true;
      planning = false;
      clearTimeout(timer);
    };
  });

  /** Requests across every selected symbol, not just the one planned. */
  const requestEstimate = $derived(
    plan === null ? null : plan.requests * Math.max(1, symbols.length),
  );

  // ── Step state helper ─────────────────────────────────────────
  type StepState = "completed" | "active" | "locked";
  function stepState(step: number): StepState {
    if (step === 1) return assetClass ? "completed" : "active";
    if (step === 2) return symbols.length > 0 ? "completed" : assetClass ? "active" : "locked";
    if (step === 3) return kindId ? "completed" : symbols.length > 0 ? "active" : "locked";
    if (step === 4) return (start && end) ? "completed" : kindId ? "active" : "locked";
    return "active";
  }

  // ── Cross-asset suggestion ─────────────────────────────────────
  // After a successful queue submit, look across asset classes for a
  // symmetric kind the user might also want (stock_history_trade →
  // option_history_trade for the same root). Like a store's "frequently
  // bought together" prompt — non-modal, dismissable, only fires for
  // kinds with a real cross-asset twin and a tier the user can access.
  type Suggestion = {
    kind: string;
    summary: string;
    category: "stock" | "option";
  };
  let suggestion = $state<Suggestion | null>(null);

  function findCrossSell(primary: string): Suggestion | null {
    let twin: string | null = null;
    if (primary.startsWith("stock_")) twin = primary.replace(/^stock_/, "option_");
    else if (primary.startsWith("option_")) twin = primary.replace(/^option_/, "stock_");
    if (!twin) return null;
    const entry = app.catalogue.find((e) => e.name === twin);
    if (!entry) return null;
    // Only suggest if the user can actually run it on their tier.
    const v = tierForKind(twin);
    if (!v.allowed) return null;
    return {
      kind: twin,
      summary: entry.summary || twin,
      category: entry.category === "stock" ? "stock" : "option",
    };
  }

  // Params that map onto a dedicated `EnqueueArgs` field. Everything
  // else the user filled in — `max_dte`, `strike_range`, `start_time`,
  // `venue`, the greeks inputs — travels in `extra` and is applied to
  // whichever of them the endpoint declares. Before this, the form
  // collected those values and the queue dropped them on the floor.
  const SPEC_FIELD_PARAMS = new Set([
    "symbol", "root", "date", "start_date", "end_date",
    "interval", "expiration", "strike", "right", "request_type",
  ]);

  /** The arguments one symbol would be queued with. Shared so the
   *  estimate plans exactly what the queue will do. */
  function enqueueArgsFor(symbol: string): EnqueueArgs {
    const isOption = kindId.startsWith("option_");
    return {
      kind: kindId,
      symbol,
      format,
      interval: interval || "tick",
      start: start || null,
      end: end || null,
      expiration: isOption ? (paramValues["expiration"] ?? null) : null,
      strike: isOption ? (paramValues["strike"] ?? null) : null,
      right:
        isOption && paramValues["right"] !== "both"
          ? (paramValues["right"] ?? null)
          : null,
      extra: extraArgs(),
    };
  }

  function extraArgs(): Record<string, string> {
    const out: Record<string, string> = {};
    for (const p of selectedParams) {
      if (SPEC_FIELD_PARAMS.has(p.name)) continue;
      const v = paramValues[p.name];
      if (v !== undefined && v !== null && String(v).trim() !== "") {
        out[p.name] = String(v);
      }
    }
    return out;
  }

  async function queueDownload() {
    if (!readyToQueue || queueStatus === "queuing") return;
    queueStatus = "queuing";
    queueMsg = `Queueing ${symbols.length} symbol${symbols.length === 1 ? "" : "s"}…`;
    suggestion = null;

    let totalTasks = 0;
    let firstErr = "";

    for (const symbol of symbols) {
      try {
        const args = enqueueArgsFor(symbol);
        const n = await api.enqueue(args);
        totalTasks += n;
      } catch (e: unknown) {
        if (!firstErr) firstErr = e instanceof Error ? e.message : String(e);
      }
    }

    // The snapshot poll backs off when the queue is idle, so an
    // enqueue has to announce itself or the pane lags behind the click.
    await refreshQueueSnapshot();

    if (firstErr && totalTasks === 0) {
      queueStatus = "error";
      queueMsg = firstErr;
      log("error", `Browse queue failed: ${firstErr}`);
    } else {
      queueStatus = "done";
      lastQueuedCount = totalTasks;
      queueMsg = `Queued ${totalTasks} task${totalTasks === 1 ? "" : "s"} · ${kindId} · ${symbols.length} symbol${symbols.length === 1 ? "" : "s"}${firstErr ? " (some failed)" : ""}`;
      log("info", `Browse: queued ${totalTasks} tasks`, {
        symbols: symbols.length,
        kind: kindId,
      });
      // Cross-sell prompt — only when there's a real symmetric twin
      // the user can access on their tier.
      suggestion = findCrossSell(kindId);
      setTimeout(() => { queueStatus = "idle"; queueMsg = ""; }, 3000);
    }
  }

  async function queueSuggestion() {
    if (!suggestion) return;
    const twin = suggestion.kind;
    const isOption = twin.startsWith("option_");
    let totalTasks = 0;
    for (const symbol of symbols) {
      try {
        const args: EnqueueArgs = {
          kind: twin,
          symbol,
          format,
          interval: interval || "tick",
          start: start || null,
          end: end || null,
          expiration: isOption ? (paramValues["expiration"] ?? "*") : null,
          strike: isOption
            ? (paramValues["strike_filter_low"] && paramValues["strike_filter_high"]
                ? `${paramValues["strike_filter_low"]}-${paramValues["strike_filter_high"]}`
                : (paramValues["strike"] ?? "*"))
            : null,
          right:
            isOption && paramValues["right"] && paramValues["right"] !== "both"
              ? (paramValues["right"] ?? null)
              : null,
          extra: extraArgs(),
        };
        const n = await api.enqueue(args);
        totalTasks += n;
      } catch {
        /* fail-silent — UI shows generic queue summary */
      }
    }
    await refreshQueueSnapshot();
    log("info", `Cross-sell: queued ${totalTasks} ${suggestion.kind} tasks`);
    suggestion = null;
  }

  function handleSavePreset() {
    if (symbols.length === 0 || !kindId) return;
    const name = `${summaryKindTitle} · ${symbols.slice(0, 3).join(", ")}${symbols.length > 3 ? ` +${symbols.length - 3}` : ""}`;
    saveSearch({ name, kind: kindId, symbols, start: start || null, end: end || null, format });
    log("info", `Saved preset: ${name}`);
  }

  async function handleUpgrade() {
    const url = app.tierStatus?.upgrade_url ?? "https://thetadata.net/pricing";
    try { await openUrl(url); } catch { /* ignore */ }
  }

  type FormatOpt = { id: "parquet" | "csv" | "jsonl" | "json"; label: string; hint: string };
  const FORMAT_OPTIONS: FormatOpt[] = [
    { id: "parquet", label: "Parquet",    hint: "Recommended — columnar, compressed" },
    { id: "csv",     label: "CSV",        hint: "Universal, larger files" },
    { id: "jsonl",   label: "JSON Lines", hint: "Streaming-friendly newline-delimited" },
    { id: "json",    label: "JSON",       hint: "Full array — best for small datasets" },
  ];
</script>

<div class="browse-page">
  <div class="page-inner">

    <!-- ── Page header ──────────────────────────────────────── -->
    <div class="page-header">
      <h1 class="page-title">Download market data</h1>
      <p class="page-sub">
        Pick an asset class, choose your symbols, select what you want, and queue.
        Takes 30 seconds.
      </p>
    </div>

    <!-- ── Step 1: Asset class ──────────────────────────────── -->
    <section class="step" aria-label="Step 1: Asset class">
      <div class="step-header">
        <span class="step-num" data-state={stepState(1)}>1</span>
        <div class="step-label-group">
          <h2 class="step-title">What asset class?</h2>
          <p class="step-sub">Choose the type of market data you want.</p>
        </div>
      </div>
      <AssetClassPicker bind:value={assetClass} />
    </section>

    <div class="step-divider" aria-hidden="true"></div>

    <!-- ── Step 2: Symbols ──────────────────────────────────── -->
    <section class="step" aria-label="Step 2: Symbols">
      <div class="step-header">
        <span class="step-num" data-state={stepState(2)}>2</span>
        <div class="step-label-group">
          <h2 class="step-title">Which symbols?</h2>
          <p class="step-sub">One ticker, an index constituent list, or a custom batch.</p>
        </div>
      </div>
      <UniverseSelector bind:symbols {assetClass} />
    </section>

    <div class="step-divider" aria-hidden="true"></div>

    <!-- ── Step 3: Data kind ────────────────────────────────── -->
    <section class="step" aria-label="Step 3: Data type">
      <div class="step-header">
        <span class="step-num" data-state={stepState(3)}>3</span>
        <div class="step-label-group">
          <h2 class="step-title">What kind of data?</h2>
          <p class="step-sub">Browse all available datasets for this asset class.</p>
        </div>
      </div>
      <KindGrid bind:selectedKindId={kindId} {assetClass} />
    </section>

    <!-- ── Step 4 (conditional): Time range ─────────────────── -->
    {#if showRange}
      <div class="step-divider" aria-hidden="true"></div>
      <section class="step" aria-label="Time range">
        <div class="step-header">
          <span class="step-num" data-state={stepState(4)}>{stepIndex("range")}</span>
          <div class="step-label-group">
            <h2 class="step-title">What time range?</h2>
            <p class="step-sub">Use a quick preset or set exact dates.</p>
          </div>
        </div>
        <RangePicker bind:start bind:end symbolCount={symbols.length} {kindId} />
      </section>
    {/if}

    <!-- ── Step (conditional): Granularity ──────────────────── -->
    {#if showInterval}
      <div class="step-divider" aria-hidden="true"></div>
      <section class="step" aria-label="Granularity">
        <div class="step-header">
          <span class="step-num">{stepIndex("interval")}</span>
          <div class="step-label-group">
            <h2 class="step-title">Granularity?</h2>
            <p class="step-sub">How finely do you want the data sampled?</p>
          </div>
        </div>
        <IntervalPicker bind:interval {kindId} />
      </section>
    {/if}

    <!-- ── Step (conditional): Filters & advanced params ────── -->
    {#if showFilters}
      <div class="step-divider" aria-hidden="true"></div>
      <section class="step" aria-label="Filters">
        <div class="step-header">
          <span class="step-num">{stepIndex("filters")}</span>
          <div class="step-label-group">
            <h2 class="step-title">Filters</h2>
            <p class="step-sub">
              Narrow the request with the optional parameters this endpoint accepts.
            </p>
          </div>
        </div>

        <!-- Smart shortcuts for well-known option params -->
        <SmartFilters
          params={selectedParams}
          symbol={symbols[0] ?? ""}
          bind:values={paramValues}
        />

        <!-- Auto-rendered form for remaining params -->
        <div class="param-form-wrap">
          <ParamForm
            params={selectedParams}
            bind:values={paramValues}
            excludeNames={[...SMART_FILTER_PARAMS]}
          />
        </div>
      </section>
    {/if}

    <div class="step-divider" aria-hidden="true"></div>

    <!-- ── Output format ─────────────────────────────────────── -->
    <section class="step" aria-label="Output format">
      <div class="step-header">
        <span class="step-num">{stepIndex("output")}</span>
        <div class="step-label-group">
          <h2 class="step-title">Output format?</h2>
          <p class="step-sub">Parquet is recommended — columnar, 3–10× smaller than CSV, fast to query.</p>
        </div>
      </div>
      <div class="format-row" role="radiogroup" aria-label="Output format">
        {#each FORMAT_OPTIONS as opt}
          <button
            type="button"
            role="radio"
            aria-checked={format === opt.id}
            class="format-btn"
            class:active={format === opt.id}
            onclick={() => (format = opt.id)}
          >
            <span class="fmt-label">
              {opt.label}
              {#if opt.id === "parquet"}
                <span class="recommended-tag">Recommended</span>
              {/if}
            </span>
            <span class="fmt-hint">{opt.hint}</span>
          </button>
        {/each}
      </div>
    </section>

    <!-- Cross-sell suggestion (post-queue) -->
    {#if suggestion}
      <section class="cross-sell" aria-label="Suggested next download">
        <div class="cross-sell-icon" aria-hidden="true">
          <Bookmark size={16} strokeWidth={1.75} />
        </div>
        <div class="cross-sell-text">
          <div class="cross-sell-title">
            You might also want <strong>{suggestion.summary}</strong> for the same symbol{symbols.length === 1 ? "" : "s"}.
          </div>
          <div class="cross-sell-sub fg-muted">
            Same date range and format — adds {suggestion.category} data for the {symbols.length} {symbols.length === 1 ? "ticker" : "tickers"} you just queued.
          </div>
        </div>
        <div class="cross-sell-actions">
          <button class="btn btn-ghost" onclick={() => (suggestion = null)}>Dismiss</button>
          <button class="btn btn-primary" onclick={queueSuggestion}>
            <Plus size={12} strokeWidth={2} />
            Queue {suggestion.summary}
          </button>
        </div>
      </section>
    {/if}

    <!-- ── Bulk surfaces ─────────────────────────────────────────
         Whole-day flat-file archives and index-constituent presets are
         separate ways in: they queue by the day or by the index rather
         than by the symbol-and-range flow above. Both have working
         runners; they were built without ever being mounted, so nothing
         could reach them. -->
    <section class="bulk-shelves" aria-label="Bulk download surfaces">
      <FlatfilesShelf />
      <IndexPresetsShelf />
    </section>

    <!-- Bottom padding so footer doesn't obscure last section -->
    <div class="footer-spacer"></div>
  </div>

  <!-- ── Sticky footer ────────────────────────────────────────── -->
  <footer class="sticky-footer">
    <div class="footer-summary">
      {#if readyToQueue}
        <span class="summary-text tabnum">
          <span class="sym-count">{symbols.length.toLocaleString()}</span>
          {symbols.length === 1 ? "symbol" : "symbols"}
          &nbsp;&middot;&nbsp;
          <span class="kind-name">{summaryKindTitle}</span>
          {#if showRange}
            &nbsp;&middot;&nbsp;
            <span class="range-val">{summaryRange}</span>
          {/if}
          &nbsp;&middot;&nbsp;
          <span class="fmt-val">{format.charAt(0).toUpperCase() + format.slice(1)}</span>
          {#if requestEstimate !== null && requestEstimate > 1}
            &nbsp;&middot;&nbsp;
            <span
              class="task-estimate"
              title={plan && plan.expirations > 1
                ? `${plan.windows} window${plan.windows === 1 ? "" : "s"} × ${plan.expirations} expirations × ${symbols.length} symbol${symbols.length === 1 ? "" : "s"} — this dataset needs one request per expiration`
                : `${plan?.windows ?? 0} window${(plan?.windows ?? 0) === 1 ? "" : "s"} × ${symbols.length} symbol${symbols.length === 1 ? "" : "s"}`}
            >
              {requestEstimate.toLocaleString()} requests
            </span>
          {:else if planning && taskEstimate > 1}
            &nbsp;&middot;&nbsp;
            <span class="task-estimate">~{taskEstimate.toLocaleString()} requests</span>
          {/if}
        </span>
      {:else if isRunOnce}
        <span class="summary-placeholder">
          {readyToRun
            ? `${summaryKindTitle} answers one question — it runs once and writes a file, rather than queueing per-day tasks.`
            : "Pick a symbol to run this lookup."}
        </span>
      {:else}
        <span class="summary-placeholder">Complete the steps above to queue a download.</span>
      {/if}
      {#if queueMsg}
        <span class="queue-msg" class:error={queueStatus === "error"}>{queueMsg}</span>
      {/if}
    </div>

    <div class="footer-actions">
      {#if symbols.length > 0 && kindId}
        <button
          type="button"
          class="btn btn-ghost save-btn"
          onclick={handleSavePreset}
          title="Save this configuration as a named preset"
        >
          <Bookmark size={14} strokeWidth={1.75} />
          Save preset
        </button>
      {/if}

      {#if gated}
        <button
          type="button"
          class="btn btn-primary queue-btn"
          onclick={handleUpgrade}
        >
          <ArrowUpRight size={16} strokeWidth={1.75} />
          Upgrade to {upgradeRequiredTier}
        </button>
      {:else if isRunOnce}
        <button
          type="button"
          class="btn btn-primary queue-btn"
          onclick={runOnce}
          disabled={!readyToRun}
          aria-label="Run endpoint once"
        >
          <Play size={16} strokeWidth={1.75} />
          Run once
        </button>
      {:else}
        <button
          type="button"
          class="btn btn-primary queue-btn"
          onclick={queueDownload}
          disabled={!readyToQueue || queueStatus === "queuing" || queueStatus === "done"}
          aria-label="Queue download"
        >
          {#if queueStatus === "queuing"}
            <Loader2 size={16} strokeWidth={1.75} class="spin" />
            Queueing…
          {:else if queueStatus === "done"}
            <Check size={16} strokeWidth={1.75} />
            Queued {lastQueuedCount} tasks
          {:else}
            Queue download
          {/if}
        </button>
      {/if}
    </div>
  </footer>
</div>

<style>
  /* ── Page shell ─────────────────────────────────────────────── */
  .browse-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    position: relative;
  }

  .page-inner {
    flex: 1;
    overflow-y: auto;
    padding: var(--sp-8) var(--sp-8) 0;
    scroll-padding-bottom: 80px;
  }

  /* ── Page header ────────────────────────────────────────────── */
  .page-header {
    margin-bottom: var(--sp-8);
  }

  .page-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    color: var(--fg);
    line-height: 1.15;
    margin-bottom: var(--sp-2);
  }

  .page-sub {
    font-size: var(--text-body);
    color: var(--fg-muted);
    max-width: 560px;
  }

  /* ── Step layout ────────────────────────────────────────────── */
  .step {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .step-header {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }

  .step-num {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    font-variant-numeric: tabular-nums;
    background: var(--surface-2);
    border: 1.5px solid var(--border);
    color: var(--fg-muted);
    transition:
      background var(--dur-base) var(--ease-standard),
      border-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard);
    margin-top: 2px;
  }

  .step-num[data-state="completed"] {
    background: var(--accent-tint);
    border-color: var(--accent);
    color: var(--accent-hi);
  }

  .step-label-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .step-title {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg);
    letter-spacing: -0.005em;
    line-height: 1.4;
  }

  .step-sub {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    line-height: 1.45;
  }

  .step-divider {
    height: 1px;
    background: var(--border);
    margin: var(--sp-6) 0;
  }

  /* Param form spacing */
  .param-form-wrap {
    margin-top: var(--sp-2);
  }

  /* ── Format radio row ───────────────────────────────────────── */
  .format-row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }

  .format-btn {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface-1);
    border: 1.5px solid var(--border);
    border-radius: var(--r-md);
    cursor: pointer;
    text-align: left;
    min-width: 140px;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
    outline: none;
  }

  .format-btn:hover:not(.active) {
    background: var(--surface-2);
    border-color: var(--border-strong);
  }

  .format-btn:focus-visible { box-shadow: var(--shadow-glow-accent); }

  .format-btn.active {
    background: var(--accent-tint);
    border-color: var(--accent);
  }

  .fmt-label {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
  }

  .format-btn.active .fmt-label {
    color: var(--accent-hi);
  }

  .fmt-hint {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
  }

  .recommended-tag {
    padding: 1px 6px;
    border-radius: var(--r-pill);
    background: var(--good-tint);
    color: var(--good);
    font-size: 10px;
    font-weight: var(--weight-semi);
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  /* ── Bottom spacer above footer ─────────────────────────────── */
  /* ── Cross-sell suggestion banner ───────────────────────── */
  .cross-sell {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    margin-top: var(--sp-4);
    background: var(--accent-tint);
    border: 1px solid var(--accent);
    border-radius: var(--r-md);
  }
  .cross-sell-icon {
    flex-shrink: 0;
    color: var(--accent);
    padding-top: 2px;
  }
  .cross-sell-text {
    flex: 1;
    min-width: 0;
  }
  .cross-sell-title {
    font-size: var(--text-body-sm);
    color: var(--fg);
    line-height: 1.4;
  }
  .cross-sell-sub {
    font-size: var(--text-caption);
    line-height: 1.4;
    margin-top: 2px;
  }
  .cross-sell-actions {
    display: flex;
    gap: var(--sp-2);
    flex-shrink: 0;
  }

  .bulk-shelves {
    display: flex;
    flex-direction: column;
    gap: var(--sp-8);
    margin-top: var(--sp-8);
    padding-top: var(--sp-8);
    border-top: 1px solid var(--border);
  }

  .footer-spacer { height: 96px; }

  /* ── Sticky footer ──────────────────────────────────────────── */
  .sticky-footer {
    position: sticky;
    bottom: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-8);
    background: var(--surface-1);
    border-top: 1px solid var(--border);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    z-index: 10;
    flex-shrink: 0;
  }

  .footer-summary {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .summary-text {
    font-size: var(--text-body-sm);
    color: var(--fg);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sym-count {
    color: var(--accent-hi);
    font-weight: var(--weight-semi);
  }

  .kind-name {
    color: var(--fg);
    font-weight: var(--weight-medium);
  }

  .range-val, .fmt-val {
    color: var(--fg-muted);
  }
  .task-estimate {
    color: var(--accent-hi);
    font-weight: var(--weight-semi);
  }

  .summary-placeholder {
    font-size: var(--text-body-sm);
    color: var(--fg-subtle);
  }

  .queue-msg {
    font-size: var(--text-body-sm);
    color: var(--good);
    transition: color var(--dur-fast) var(--ease-standard);
  }

  .queue-msg.error { color: var(--bad); }

  .footer-actions {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex-shrink: 0;
  }

  .save-btn {
    height: 36px;
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
  }

  .save-btn:hover { color: var(--fg); }

  .queue-btn {
    height: 40px;
    padding: 0 var(--sp-5);
    font-size: var(--text-body);
    font-weight: var(--weight-semi);
    border-radius: var(--r-md);
    gap: var(--sp-2);
  }

  .queue-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  :global(.queue-btn .spin) {
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }
</style>
