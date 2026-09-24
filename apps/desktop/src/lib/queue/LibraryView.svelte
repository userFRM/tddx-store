<script lang="ts">
  import {
    Search,
    ChevronDown,
    ChevronRight,
    FolderOpen,
    RotateCcw,
    Plus,
    ArrowRight,
    Library,
    Diff,
    Database,
    LineChart,
    Trash2,
    Copy,
    X,
    FileText,
  } from "lucide-svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { api, fmtBytes, fmtNum, type Coverage, type LibraryFile, type LibraryTargets } from "$lib/api";
  import { app, navigate, log, refreshQueueSnapshot, browseTo, loadCoverage, openViewer } from "$lib/stores/app.svelte";
  import { onMount } from "svelte";
  import CoverageDiff from "$lib/queue/CoverageDiff.svelte";
  import ContextMenu, { type MenuItem } from "$lib/queue/ContextMenu.svelte";
  import ConfirmTrash from "$lib/queue/ConfirmTrash.svelte";

  type SortKey = "symbol" | "size" | "files" | "recent";

  const SORT_LABELS: Record<SortKey, string> = {
    symbol: "Symbol A-Z",
    size: "Largest first",
    files: "Most files",
    recent: "Most recent",
  };

  const coverage = $derived(app.coverage);
  const loading = $derived(app.coverageLoading && app.coverage.length === 0);
  let rowMsg = $state("");
  let sortKey = $state<SortKey>("symbol");
  let busy = $state(false);

  /** Queue every trading day missing from a set's own span. */
  async function refillGaps(row: Coverage) {
    // The count is now queued work, so the panel's copy of it is stale.
    const key = gapKey(row);
    const { [key]: _dropped, ...rest } = gaps;
    gaps = rest;
    openGaps = new Set([...openGaps].filter((k) => k !== key));
    rowMsg = `Checking ${row.symbol} ${row.kind}…`;
    try {
      const n = await api.requeueMissingDates(row.kind, row.symbol);
      await refreshQueueSnapshot();
      rowMsg =
        n === 0
          ? `${row.symbol} ${row.kind} has no gaps`
          : `Queued ${n} missing day${n === 1 ? "" : "s"} for ${row.symbol}`;
      log("info", rowMsg);
    } catch (e: unknown) {
      rowMsg = e instanceof Error ? e.message : String(e);
      log("error", `Refill failed: ${rowMsg}`);
    }
    setTimeout(() => (rowMsg = ""), 3000);
  }

  /** Copy a DuckDB bootstrap script that exposes every dataset on disk
   *  as a queryable view. A downloader whose output cannot be opened is
   *  half a tool; this is the shortest path from "downloaded" to
   *  "queried" without the app growing a SQL console. */
  async function copyDuckDbScript() {
    busy = true;
    try {
      const { sql } = await api.duckdbCommand(app.settings.output_dir);
      await writeText(sql);
      rowMsg = "DuckDB script copied — paste it into a duckdb session";
      log("info", rowMsg);
    } catch (e: unknown) {
      rowMsg = e instanceof Error ? e.message : String(e);
      log("error", `DuckDB export failed: ${rowMsg}`);
    } finally {
      busy = false;
      setTimeout(() => (rowMsg = ""), 4000);
    }
  }

  /** Reveal the dataset's directory in the OS file manager. */
  async function revealKindDir(row: Coverage) {
    try {
      await revealItemInDir(`${app.settings.output_dir}/${row.kind}`);
    } catch (e: unknown) {
      rowMsg = e instanceof Error ? e.message : String(e);
      setTimeout(() => (rowMsg = ""), 3000);
    }
  }
  /** "What landed since I last looked" — a snapshot diff over the
   *  same coverage rows, opened on demand so it costs nothing when
   *  closed. */
  let showDiff = $state(false);
  let filterQuery = $state("");
  let expandedSymbols = $state<Set<string>>(new Set());

  // Shared with Home, and invalidated by the queue poll when a task
  // finishes, so the numbers here do not go stale behind a download.
  onMount(() => void loadCoverage());

  // Group by symbol, filtering on the symbol AND the dataset, because
  // "show me everything with greeks" is as common a question as
  // "show me everything for QQQ".
  const grouped = $derived.by<[string, Coverage[]][]>(() => {
    const q = filterQuery.trim().toLowerCase();
    const map = new Map<string, Coverage[]>();
    for (const row of coverage) {
      if (
        q &&
        !row.symbol.toLowerCase().includes(q) &&
        !row.kind.toLowerCase().includes(q) &&
        !kindLabel(row.kind).toLowerCase().includes(q)
      ) {
        continue;
      }
      if (!map.has(row.symbol)) map.set(row.symbol, []);
      map.get(row.symbol)!.push(row);
    }
    const groups = Array.from(map.entries());
    groups.sort(([aSym, aRows], [bSym, bRows]) => {
      switch (sortKey) {
        case "size":
          return symbolTotalBytes(bRows) - symbolTotalBytes(aRows);
        case "files":
          return symbolTotalFiles(bRows) - symbolTotalFiles(aRows);
        case "recent":
          return (lastDate(bRows) ?? "").localeCompare(lastDate(aRows) ?? "");
        default:
          return aSym.localeCompare(bSym);
      }
    });
    return groups;
  });

  const allExpanded = $derived(
    grouped.length > 0 && grouped.every(([sym]) => expandedSymbols.has(sym)),
  );

  function toggleAll() {
    expandedSymbols = allExpanded
      ? new Set()
      : new Set(grouped.map(([sym]) => sym));
  }

  function lastDate(rows: Coverage[]): string | null {
    return rows.map((r) => r.last).filter(Boolean).sort().pop() ?? null;
  }

  /** Per-row gap state, keyed `kind|symbol`. Absent until the user
   *  asks: the answer needs the vendor's trading calendar, so it is a
   *  network call, not something to fire for every visible row. */
  type GapState =
    | { status: "loading" }
    | { status: "error"; message: string }
    | { status: "ready"; dates: string[] };
  let gaps = $state<Record<string, GapState>>({});
  let openGaps = $state<Set<string>>(new Set());

  const gapKey = (row: Coverage) => `${row.kind}|${row.symbol}`;

  /** Collapse a date list into contiguous runs, so 33 scattered days
   *  read as a handful of ranges instead of a wall of dates. */
  function toRanges(dates: string[]): { from: string; to: string; days: number }[] {
    const out: { from: string; to: string; days: number }[] = [];
    for (const date of dates) {
      const prev = out[out.length - 1];
      const dayAfter = prev
        ? new Date(new Date(prev.to + "T00:00:00Z").getTime() + 86_400_000)
            .toISOString()
            .slice(0, 10)
        : null;
      // Runs join across a weekend, since Saturday and Sunday are not
      // gaps and would otherwise split every week into its own range.
      const within = prev && date > prev.to && date <= addDays(prev.to, 3);
      if (prev && (date === dayAfter || within)) {
        prev.to = date;
        prev.days += 1;
      } else {
        out.push({ from: date, to: date, days: 1 });
      }
    }
    return out;
  }

  function addDays(iso: string, n: number): string {
    return new Date(new Date(iso + "T00:00:00Z").getTime() + n * 86_400_000)
      .toISOString()
      .slice(0, 10);
  }

  async function toggleGaps(row: Coverage) {
    const key = gapKey(row);
    const next = new Set(openGaps);
    if (next.has(key)) {
      next.delete(key);
      openGaps = next;
      return;
    }
    next.add(key);
    openGaps = next;
    if (gaps[key]?.status === "ready") return;
    gaps = { ...gaps, [key]: { status: "loading" } };
    try {
      const dates = await api.missingDates(row.kind, row.symbol);
      gaps = { ...gaps, [key]: { status: "ready", dates } };
    } catch (e: unknown) {
      gaps = {
        ...gaps,
        [key]: { status: "error", message: e instanceof Error ? e.message : String(e) },
      };
    }
  }

  /** Fill gaps across every dataset held for one symbol. */
  async function refillSymbol(symbol: string, rows: Coverage[]) {
    if (busy) return;
    busy = true;
    rowMsg = `Checking ${symbol}…`;
    try {
      let queued = 0;
      for (const row of rows) {
        queued += await api.requeueMissingDates(row.kind, row.symbol);
      }
      rowMsg =
        queued === 0
          ? `${symbol} has no gaps`
          : `Queued ${queued} missing day${queued === 1 ? "" : "s"} for ${symbol}`;
      log("info", rowMsg);
    } catch (e: unknown) {
      rowMsg = e instanceof Error ? e.message : String(e);
      log("error", `Refill failed: ${rowMsg}`);
    } finally {
      busy = false;
      setTimeout(() => (rowMsg = ""), 3000);
    }
  }

  function toggleSymbol(symbol: string) {
    const s = new Set(expandedSymbols);
    if (s.has(symbol)) s.delete(symbol);
    else s.add(symbol);
    expandedSymbols = s;
  }

  function symbolTotalBytes(rows: Coverage[]): number {
    return rows.reduce((sum, r) => sum + r.bytes, 0);
  }
  function symbolTotalFiles(rows: Coverage[]): number {
    return rows.reduce((sum, r) => sum + r.files, 0);
  }
  function symbolSpan(rows: Coverage[]): string {
    const firsts = rows.map((r) => r.first).filter(Boolean) as string[];
    const lasts  = rows.map((r) => r.last).filter(Boolean) as string[];
    if (firsts.length === 0) return "—";
    const first = firsts.sort()[0];
    const last  = lasts.sort().reverse()[0];
    return `${first} → ${last}`;
  }

  // Kind label from catalogue or coverage kind string
  function kindLabel(kind: string): string {
    const entry = app.catalogue.find((e) => e.name === kind);
    if (entry) return entry.summary || kind;
    // Fallback: humanise snake_case
    return kind.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  }

  // ── Never-downloaded section ──────────────────────────────────
  // Show catalogue entries that have no coverage rows
  const downloadedKinds = $derived(new Set(coverage.map((r) => r.kind)));

  const neverDownloaded = $derived.by(() => {
    if (app.catalogue.length === 0) return [];
    return app.catalogue.filter((e) => !downloadedKinds.has(e.name));
  });

  // Group never-downloaded by category
  const neverByCategory = $derived.by(() => {
    const order: string[] = [];
    const map = new Map<string, typeof app.catalogue>();
    for (const e of neverDownloaded) {
      const cat = e.category.charAt(0).toUpperCase() + e.category.slice(1);
      if (!map.has(cat)) { map.set(cat, []); order.push(cat); }
      map.get(cat)!.push(e);
    }
    return order.map((c) => ({ category: c, entries: map.get(c)! }));
  });

  let neverOpen = $state(false);

  // First sentence of description
  function firstSentence(desc: string): string {
    if (!desc) return "";
    const m = desc.match(/^(.+?[.!?])\s/s);
    return m ? m[1] : desc.slice(0, 100);
  }
  // ── Finder-style tree: symbol → dataset → file ─────────────────
  //
  // Every row is an item with a key; selection is a set of keys, so a
  // symbol, a dataset and a single file can be selected together and
  // acted on at once. Files load when their dataset is opened, not up
  // front: a few years of daily files is thousands of rows.

  type Item =
    | { type: "symbol"; key: string; symbol: string; rows: Coverage[] }
    | { type: "set"; key: string; symbol: string; row: Coverage }
    | { type: "file"; key: string; symbol: string; row: Coverage; file: LibraryFile };

  const symKey = (symbol: string) => `s:${symbol}`;
  const setKey = (row: Coverage) => `d:${row.kind}|${row.symbol}`;
  const fileKey = (f: LibraryFile) => `f:${f.path}`;

  type FilesState =
    | { status: "loading" }
    | { status: "error"; message: string }
    | { status: "ready"; files: LibraryFile[] };
  let expandedSets = $state<Set<string>>(new Set());
  let filesBySet = $state<Record<string, FilesState>>({});

  let selected = $state<Set<string>>(new Set());
  let anchor = $state<string | null>(null);
  let cursor = $state<string | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);

  /** Rows in on-screen order: what arrow keys and shift-click walk. */
  const visible = $derived.by<Item[]>(() => {
    const out: Item[] = [];
    for (const [symbol, rows] of grouped) {
      out.push({ type: "symbol", key: symKey(symbol), symbol, rows });
      if (!expandedSymbols.has(symbol)) continue;
      for (const row of rows) {
        const k = setKey(row);
        out.push({ type: "set", key: k, symbol, row });
        const fs = filesBySet[k];
        if (expandedSets.has(k) && fs?.status === "ready") {
          for (const file of fs.files) {
            out.push({ type: "file", key: fileKey(file), symbol, row, file });
          }
        }
      }
    }
    return out;
  });
  const byKey = $derived(new Map(visible.map((it) => [it.key, it])));

  // A filter or a collapse can hide selected rows; acting on rows the
  // user can no longer see would be a surprise, so they drop out.
  $effect(() => {
    const keep = [...selected].filter((k) => byKey.has(k));
    if (keep.length !== selected.size) selected = new Set(keep);
  });

  const selectedItems = $derived(visible.filter((it) => selected.has(it.key)));

  /** Files a selection covers without double counting a file whose
   *  dataset, or a dataset whose symbol, is also selected. */
  const selectionTotals = $derived.by(() => {
    let files = 0;
    let bytes = 0;
    for (const it of selectedItems) {
      if (it.type === "symbol") {
        files += symbolTotalFiles(it.rows);
        bytes += symbolTotalBytes(it.rows);
      } else if (it.type === "set") {
        if (selected.has(symKey(it.symbol))) continue;
        files += it.row.files;
        bytes += it.row.bytes;
      } else {
        if (selected.has(symKey(it.symbol)) || selected.has(setKey(it.row))) continue;
        files += 1;
        bytes += it.file.bytes;
      }
    }
    return { files, bytes };
  });

  function targetsOf(items: Item[]): LibraryTargets {
    const sets: LibraryTargets["sets"] = [];
    const files: string[] = [];
    for (const it of items) {
      if (it.type === "symbol") for (const r of it.rows) sets.push({ kind: r.kind, symbol: r.symbol });
      else if (it.type === "set") sets.push({ kind: it.row.kind, symbol: it.row.symbol });
      else files.push(it.file.path);
    }
    return { sets, files };
  }

  function itemName(it: Item): string {
    if (it.type === "symbol") return it.symbol;
    if (it.type === "set") return `${it.symbol} ${kindLabel(it.row.kind)}`;
    return it.file.name;
  }

  // ── Selection ───────────────────────────────────────────────────
  function rangeKeys(from: string, to: string): string[] {
    const a = visible.findIndex((it) => it.key === from);
    const b = visible.findIndex((it) => it.key === to);
    if (a < 0 || b < 0) return [to];
    const [lo, hi] = a < b ? [a, b] : [b, a];
    return visible.slice(lo, hi + 1).map((it) => it.key);
  }

  function selectFrom(e: MouseEvent | KeyboardEvent, key: string) {
    const additive = e.metaKey || e.ctrlKey;
    if (e.shiftKey && anchor) {
      const range = rangeKeys(anchor, key);
      selected = new Set(additive ? [...selected, ...range] : range);
    } else if (additive) {
      const next = new Set(selected);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      selected = next;
      anchor = key;
    } else {
      selected = new Set([key]);
      anchor = key;
    }
    cursor = key;
  }

  function onRowClick(e: MouseEvent, key: string) {
    selectFrom(e, key);
    listEl?.focus({ preventScroll: true });
  }

  function onCheck(e: MouseEvent, key: string) {
    e.stopPropagation();
    const next = new Set(selected);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    selected = next;
    anchor = key;
    cursor = key;
  }

  function selectAll() {
    selected = new Set(visible.map((it) => it.key));
  }
  function clearSelection() {
    selected = new Set();
    anchor = null;
  }

  // ── Expand / open ───────────────────────────────────────────────
  async function toggleSet(row: Coverage, open?: boolean) {
    const k = setKey(row);
    const next = new Set(expandedSets);
    const willOpen = open ?? !next.has(k);
    if (willOpen) next.add(k);
    else next.delete(k);
    expandedSets = next;
    if (willOpen && filesBySet[k]?.status !== "ready") await loadFiles(row);
  }

  async function loadFiles(row: Coverage) {
    const k = setKey(row);
    filesBySet = { ...filesBySet, [k]: { status: "loading" } };
    try {
      const files = await api.libraryFiles(row.kind, row.symbol);
      filesBySet = { ...filesBySet, [k]: { status: "ready", files } };
    } catch (e: unknown) {
      filesBySet = {
        ...filesBySet,
        [k]: { status: "error", message: e instanceof Error ? e.message : String(e) },
      };
    }
  }

  function setExpanded(it: Item, open: boolean) {
    if (it.type === "symbol") {
      if (expandedSymbols.has(it.symbol) !== open) toggleSymbol(it.symbol);
    } else if (it.type === "set") {
      void toggleSet(it.row, open);
    }
  }

  function isExpanded(it: Item): boolean {
    if (it.type === "symbol") return expandedSymbols.has(it.symbol);
    if (it.type === "set") return expandedSets.has(it.key);
    return false;
  }

  /** Enter / double-click: folders open, files show. */
  function openItem(it: Item) {
    if (it.type === "file") quickLook(it);
    else setExpanded(it, !isExpanded(it));
  }

  /** Space: look inside without leaving the list. */
  function quickLook(it: Item) {
    if (it.type === "file") {
      if (it.file.format === "parquet") {
        openViewer(it.file.path, `${it.symbol} · ${kindLabel(it.row.kind)} · ${it.file.start}`);
      } else {
        void revealItemInDir(it.file.path);
      }
    } else if (it.type === "set" && it.row.latest_path) {
      openViewer(it.row.latest_path, `${it.symbol} · ${kindLabel(it.row.kind)}`);
    } else if (it.type === "symbol") {
      setExpanded(it, true);
    }
  }

  // ── Actions on the selection ───────────────────────────────────
  function flash(msg: string, ms = 3000) {
    rowMsg = msg;
    setTimeout(() => { if (rowMsg === msg) rowMsg = ""; }, ms);
  }

  async function revealItems(items: Item[]) {
    const paths: string[] = [];
    for (const it of items) {
      if (it.type === "file") paths.push(it.file.path);
      else if (it.type === "set" && it.row.latest_path) paths.push(it.row.latest_path);
      else if (it.type === "symbol")
        for (const r of it.rows) if (r.latest_path) paths.push(r.latest_path);
    }
    if (paths.length === 0) return;
    try {
      await revealItemInDir([...new Set(paths)]);
    } catch (e: unknown) {
      flash(e instanceof Error ? e.message : String(e));
    }
  }

  /** Files by path; whole datasets as a glob, which is what DuckDB,
   *  polars and a shell all take. */
  async function copyPaths(items: Item[]) {
    const out = app.settings.output_dir.replace(/\/$/, "");
    const lines = new Set<string>();
    const glob = (r: Coverage) => `${out}/${r.kind}/${r.symbol.toLowerCase()}_${r.kind}_*.${r.format}`;
    for (const it of items) {
      if (it.type === "file") lines.add(it.file.path);
      else if (it.type === "set") lines.add(glob(it.row));
      else for (const r of it.rows) lines.add(glob(r));
    }
    await writeText([...lines].join("\n"));
    flash(`Copied ${lines.size} ${lines.size === 1 ? "path" : "paths"}`);
  }

  async function fillGapsFor(items: Item[]) {
    const rows = new Map<string, Coverage>();
    for (const it of items) {
      if (it.type === "symbol") for (const r of it.rows) rows.set(setKey(r), r);
      else rows.set(setKey(it.row), it.row);
    }
    if (busy || rows.size === 0) return;
    busy = true;
    rowMsg = `Checking ${rows.size} ${rows.size === 1 ? "dataset" : "datasets"}…`;
    try {
      let queued = 0;
      for (const r of rows.values()) queued += await api.requeueMissingDates(r.kind, r.symbol);
      await refreshQueueSnapshot();
      flash(queued === 0 ? "No gaps to fill" : `Queued ${fmtNum(queued)} missing ${queued === 1 ? "day" : "days"}`);
      log("info", rowMsg);
    } catch (e: unknown) {
      flash(e instanceof Error ? e.message : String(e));
      log("error", `Refill failed: ${rowMsg}`);
    } finally {
      busy = false;
    }
  }

  let trashItems = $state<Item[] | null>(null);

  function askTrash(items: Item[]) {
    if (items.length > 0) trashItems = items;
  }

  async function doTrash() {
    if (!trashItems) return;
    const items = trashItems;
    const res = await api.libraryTrash(targetsOf(items));
    trashItems = null;
    // Listings under anything touched are stale now.
    const touched = new Set<string>();
    for (const it of items) {
      if (it.type === "symbol") for (const r of it.rows) touched.add(setKey(r));
      else touched.add(setKey(it.row));
    }
    const rest = { ...filesBySet };
    for (const k of touched) delete rest[k];
    filesBySet = rest;
    clearSelection();
    // Home and the Queue tab show the footprint too; refresh them now
    // rather than on their next poll.
    await Promise.all([loadCoverage(true), refreshQueueSnapshot()]);
    for (const it of items) {
      if (it.type !== "file") continue;
      const k = setKey(it.row);
      if (expandedSets.has(k) && app.coverage.some((c) => setKey(c) === k)) void loadFiles(it.row);
    }
    const msg = `Moved ${fmtNum(res.files)} ${res.files === 1 ? "file" : "files"} (${fmtBytes(res.bytes)}) to the Trash`;
    flash(msg, 5000);
    log("info", msg);
  }

  // ── Context menu ────────────────────────────────────────────────
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  function onContext(e: MouseEvent, key: string) {
    e.preventDefault();
    // Finder: right-clicking outside the selection selects just that.
    if (!selected.has(key)) {
      selected = new Set([key]);
      anchor = key;
    }
    cursor = key;
    const items = visible.filter((it) => selected.has(it.key));
    const one = items.length === 1 ? items[0] : null;
    const n = items.length;
    const menuItems: MenuItem[] = [];
    if (one) {
      menuItems.push({
        label: one.type === "file" ? "Open" : isExpanded(one) ? "Collapse" : "Expand",
        shortcut: "↩",
        run: () => openItem(one),
      });
      if (one.type !== "symbol")
        menuItems.push({ label: "Quick Look", shortcut: "Space", run: () => quickLook(one) });
      if (one.type === "set")
        menuItems.push({ label: "Download more dates…", run: () => browseTo(one.row.kind, one.symbol) });
      menuItems.push({ separator: true });
    }
    menuItems.push(
      { label: "Reveal in Finder", run: () => revealItems(items) },
      { label: n === 1 ? "Copy path" : `Copy ${n} paths`, shortcut: "⌘C", run: () => copyPaths(items) },
      {
        label: "Fill gaps",
        disabled: busy || items.every((it) => it.type === "file"),
        run: () => fillGapsFor(items),
      },
      { separator: true },
      { label: n === 1 ? "Move to Trash" : `Move ${n} items to Trash`, shortcut: "⌘⌫", danger: true, run: () => askTrash(items) },
    );
    menu = { x: e.clientX, y: e.clientY, items: menuItems };
  }

  // ── Keyboard ────────────────────────────────────────────────────
  function onListKey(e: KeyboardEvent) {
    if (trashItems || menu) return;
    const mod = e.metaKey || e.ctrlKey;
    const idx = cursor ? visible.findIndex((it) => it.key === cursor) : -1;
    const cur = idx >= 0 ? visible[idx] : null;

    const moveTo = (i: number) => {
      const it = visible[Math.max(0, Math.min(visible.length - 1, i))];
      if (!it) return;
      if (e.shiftKey && anchor) {
        selected = new Set(rangeKeys(anchor, it.key));
        cursor = it.key;
      } else {
        selectFrom(e, it.key);
      }
      document.getElementById(rowId(it.key))?.scrollIntoView({ block: "nearest" });
    };

    if (e.key === "ArrowDown") { e.preventDefault(); moveTo(idx + 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); moveTo(idx < 0 ? 0 : idx - 1); }
    else if (e.key === "Home") { e.preventDefault(); moveTo(0); }
    else if (e.key === "End") { e.preventDefault(); moveTo(visible.length - 1); }
    else if (e.key === "ArrowRight" && cur) {
      e.preventDefault();
      if (cur.type !== "file" && !isExpanded(cur)) setExpanded(cur, true);
      else moveTo(idx + 1);
    } else if (e.key === "ArrowLeft" && cur) {
      e.preventDefault();
      if (cur.type !== "file" && isExpanded(cur)) setExpanded(cur, false);
      else {
        // To the parent row, as Finder does.
        const parent =
          cur.type === "file" ? byKey.get(setKey(cur.row))
          : cur.type === "set" ? byKey.get(symKey(cur.symbol))
          : null;
        if (parent) moveTo(visible.indexOf(parent));
      }
    } else if (e.key === "Enter" && cur) { e.preventDefault(); openItem(cur); }
    else if (e.key === " " && cur) { e.preventDefault(); quickLook(cur); }
    else if (mod && e.key.toLowerCase() === "a") { e.preventDefault(); selectAll(); }
    else if (mod && e.key.toLowerCase() === "c" && selectedItems.length) { e.preventDefault(); void copyPaths(selectedItems); }
    else if ((e.key === "Backspace" && mod) || e.key === "Delete") { e.preventDefault(); askTrash(selectedItems); }
    else if (e.key === "Escape") { e.preventDefault(); clearSelection(); }
  }

  const rowId = (key: string) => `lib-row-${key.replace(/[^a-zA-Z0-9_-]/g, "_")}`;

  function fmtModified(ms: number | null): string {
    if (ms == null) return "—";
    const d = new Date(ms);
    return `${d.toISOString().slice(0, 10)} ${d.toTimeString().slice(0, 5)}`;
  }
</script>

<div class="library-view">
  <!-- Header -->
  <div class="lib-header">
    <div class="header-left">
      <h1 class="lib-title">Library</h1>
      {#if coverage.length > 0}
        <span class="lib-meta text-figures fg-muted">
          {grouped.length} {grouped.length === 1 ? "symbol" : "symbols"} · {fmtBytes(coverage.reduce((s, r) => s + r.bytes, 0))} total
        </span>
      {/if}
    </div>

    {#if rowMsg}
      <span class="row-msg text-body-sm" role="status">{rowMsg}</span>
    {/if}

    <div class="header-controls">
      {#if grouped.length > 0}
        <button class="btn btn-ghost" onclick={toggleAll}>
          {allExpanded ? "Collapse all" : "Expand all"}
        </button>
      {/if}

      {#if coverage.length > 0}
        <button
          class="btn btn-ghost"
          onclick={copyDuckDbScript}
          disabled={busy}
          title="Copy a DuckDB script that views every dataset on disk"
        >
          <Database size={14} strokeWidth={1.75} aria-hidden="true" />
          DuckDB
        </button>
      {/if}

      <button
        class="btn btn-ghost"
        class:active={showDiff}
        onclick={() => (showDiff = !showDiff)}
        aria-pressed={showDiff}
        title="Compare the library against a saved snapshot"
      >
        <Diff size={14} strokeWidth={1.75} aria-hidden="true" />
        Changes
      </button>

      <label class="sort-control">
        <span class="sr-only">Sort by</span>
        <select class="sort-select" bind:value={sortKey} aria-label="Sort by">
          {#each Object.entries(SORT_LABELS) as [key, label]}
            <option value={key}>{label}</option>
          {/each}
        </select>
      </label>

      <div class="search-wrap">
        <Search size={14} strokeWidth={1.75} class="search-icon" aria-hidden="true" />
        <input
          class="search-input"
          type="search"
          placeholder="Filter symbol or dataset…"
          bind:value={filterQuery}
          aria-label="Filter by symbol or dataset"
        />
      </div>
    </div>
  </div>

  {#if showDiff}
    <div class="diff-panel"><CoverageDiff /></div>
  {/if}

  {#if selectedItems.length > 0}
    <div class="selection-bar" role="toolbar" aria-label="Selection">
      <span class="sel-count text-figures">
        <strong>{fmtNum(selectedItems.length)}</strong> selected
        <span class="fg-subtle">·</span>
        {fmtNum(selectionTotals.files)} {selectionTotals.files === 1 ? "file" : "files"}
        <span class="fg-subtle">·</span>
        {fmtBytes(selectionTotals.bytes)}
      </span>
      <div class="sel-actions">
        <button class="btn btn-ghost btn-sm" onclick={() => revealItems(selectedItems)}>
          <FolderOpen size={13} strokeWidth={1.75} /> Reveal
        </button>
        <button class="btn btn-ghost btn-sm" onclick={() => copyPaths(selectedItems)} title="⌘C">
          <Copy size={13} strokeWidth={1.75} /> Copy paths
        </button>
        <button
          class="btn btn-ghost btn-sm"
          onclick={() => fillGapsFor(selectedItems)}
          disabled={busy || selectedItems.every((it) => it.type === "file")}
        >
          <RotateCcw size={13} strokeWidth={1.75} /> Fill gaps
        </button>
        <button class="btn btn-sm sel-trash" onclick={() => askTrash(selectedItems)} title="⌘⌫">
          <Trash2 size={13} strokeWidth={1.75} /> Move to Trash
        </button>
        <button class="btn-icon" onclick={clearSelection} aria-label="Clear selection" title="Esc">
          <X size={14} />
        </button>
      </div>
    </div>
  {/if}

  <!-- Content -->
  <div class="lib-body">
    {#if loading}
      <div class="loading-state">
        <div class="spinner" aria-label="Loading library"></div>
        <span class="text-body-sm fg-muted">Loading library…</span>
      </div>
    {:else if grouped.length === 0 && !filterQuery}
      <div class="empty-state">
        <div class="empty-icon" aria-hidden="true">
          <Library size={40} strokeWidth={1.25} />
        </div>
        <p class="empty-label">Library is empty</p>
        <p class="text-body-sm fg-muted">Download datasets from Browse to see them here.</p>
        <button
          type="button"
          class="btn btn-primary empty-cta"
          onclick={() => navigate("browse")}
        >
          Browse datasets
          <ArrowRight size={14} strokeWidth={1.75} />
        </button>
      </div>
    {:else}
      <!-- Downloaded symbol list -->
      {#if grouped.length > 0}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div
          class="symbol-list"
          role="tree"
          aria-label="Library"
          aria-multiselectable="true"
          tabindex="0"
          bind:this={listEl}
          onkeydown={onListKey}
        >
          <div class="list-head text-caption">
            <input
              type="checkbox"
              class="row-check"
              aria-label="Select all"
              checked={selected.size > 0 && selected.size === visible.length}
              indeterminate={selected.size > 0 && selected.size < visible.length}
              onchange={() => (selected.size === visible.length ? clearSelection() : selectAll())}
            />
            <span>Name</span>
            <span class="list-hint fg-subtle">Click to select · ⌘ or ⇧ for more · right-click for actions</span>
          </div>
          {#each grouped as [symbol, rows] (symbol)}
            {@const expanded = expandedSymbols.has(symbol)}
            {@const sk = symKey(symbol)}
            <div class="symbol-group" role="none">
              <div
                id={rowId(sk)}
                class="symbol-head"
                class:selected={selected.has(sk)}
                class:cursor={cursor === sk}
                role="treeitem"
                aria-level={1}
                aria-selected={selected.has(sk)}
                aria-expanded={expanded}
                tabindex="-1"
                onclick={(e) => onRowClick(e, sk)}
                ondblclick={() => toggleSymbol(symbol)}
                oncontextmenu={(e) => onContext(e, sk)}
                onkeydown={() => {}}
              >
                <input
                  type="checkbox"
                  class="row-check"
                  aria-label="Select {symbol}"
                  checked={selected.has(sk)}
                  onclick={(e) => onCheck(e, sk)}
                />
                <button
                  class="chevron-btn"
                  tabindex="-1"
                  onclick={(e) => { e.stopPropagation(); toggleSymbol(symbol); }}
                  aria-label={expanded ? `Collapse ${symbol}` : `Expand ${symbol}`}
                >
                  {#if expanded}<ChevronDown size={14} strokeWidth={1.75} />{:else}<ChevronRight size={14} strokeWidth={1.75} />{/if}
                </button>
                <span class="symbol-ticker text-figures">{symbol}</span>
                <div class="symbol-summary">
                  <span class="sum-stat text-figures">{fmtNum(rows.length)} {rows.length === 1 ? "dataset" : "datasets"}</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-figures">{fmtNum(symbolTotalFiles(rows))} files</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-figures">{fmtBytes(symbolTotalBytes(rows))}</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-figures">{symbolSpan(rows)}</span>
                </div>
                <div class="symbol-aside">
                  <button
                    class="btn btn-secondary btn-sm"
                    tabindex="-1"
                    onclick={(e) => { e.stopPropagation(); refillSymbol(symbol, rows); }}
                    disabled={busy}
                    title="Check every dataset for {symbol} and queue whatever is missing"
                  >
                    <RotateCcw size={12} strokeWidth={1.75} />
                    Fill gaps
                  </button>
                </div>
              </div>

              {#if expanded}
                <div class="kind-rows" role="group">
                  {#each rows as row (row.kind)}
                    {@const dk = setKey(row)}
                    {@const setOpen = expandedSets.has(dk)}
                    {@const fs = filesBySet[dk]}
                    <div
                      id={rowId(dk)}
                      class="kind-row"
                      class:selected={selected.has(dk)}
                      class:cursor={cursor === dk}
                      role="treeitem"
                      aria-level={2}
                      aria-selected={selected.has(dk)}
                      aria-expanded={setOpen}
                      tabindex="-1"
                      onclick={(e) => onRowClick(e, dk)}
                      ondblclick={() => toggleSet(row)}
                      oncontextmenu={(e) => onContext(e, dk)}
                      onkeydown={() => {}}
                    >
                      <div class="kind-lead">
                        <input
                          type="checkbox"
                          class="row-check"
                          aria-label="Select {symbol} {row.kind}"
                          checked={selected.has(dk)}
                          onclick={(e) => onCheck(e, dk)}
                        />
                        <button
                          class="chevron-btn"
                          tabindex="-1"
                          onclick={(e) => { e.stopPropagation(); toggleSet(row); }}
                          aria-label={setOpen ? "Hide files" : "Show files"}
                        >
                          {#if setOpen}<ChevronDown size={13} strokeWidth={1.75} />{:else}<ChevronRight size={13} strokeWidth={1.75} />{/if}
                        </button>
                        <div class="kind-info">
                          <code class="kind-name" title={row.kind}>{row.kind}</code>
                          <span class="kind-title text-body-sm fg-muted">{kindLabel(row.kind)}</span>
                        </div>
                      </div>
                      <div class="kind-stats text-figures">
                        <span>{fmtNum(row.files)} files</span>
                        <span class="sum-sep">·</span>
                        <span>{fmtBytes(row.bytes)}</span>
                        <span class="sum-sep">·</span>
                        <span>{row.format}</span>
                        <span class="sum-sep">·</span>
                        <span>{row.first ?? "—"} → {row.last ?? "—"}</span>
                      </div>
                      <button
                        class="gap-toggle"
                        class:open={openGaps.has(gapKey(row))}
                        tabindex="-1"
                        onclick={(e) => { e.stopPropagation(); toggleGaps(row); }}
                        aria-expanded={openGaps.has(gapKey(row))}
                      >
                        {#if gaps[gapKey(row)]?.status === "loading"}
                          Checking…
                        {:else if gaps[gapKey(row)]?.status === "ready"}
                          {@const n = (gaps[gapKey(row)] as { dates: string[] }).dates.length}
                          {n === 0 ? "Complete" : `${fmtNum(n)} missing`}
                        {:else}
                          Check gaps
                        {/if}
                      </button>

                      <div class="kind-actions">
                        <button
                          class="btn-icon"
                          tabindex="-1"
                          onclick={(e) => { e.stopPropagation(); browseTo(row.kind, symbol); }}
                          title="Download more dates"
                          aria-label="Download more dates for {symbol} {row.kind}"
                        >
                          <Plus size={13} strokeWidth={1.75} />
                        </button>
                        <button
                          class="btn-icon"
                          tabindex="-1"
                          onclick={(e) => { e.stopPropagation(); refillGaps(row); }}
                          title="Queue the missing dates in this range"
                          aria-label="Queue the missing dates for {symbol} {row.kind}"
                        >
                          <RotateCcw size={13} strokeWidth={1.75} />
                        </button>
                        {#if row.latest_path}
                          <button
                            class="btn-icon"
                            tabindex="-1"
                            onclick={(e) => { e.stopPropagation(); openViewer(row.latest_path!, `${symbol} · ${kindLabel(row.kind)}`); }}
                            title="View the latest file — chart and rows"
                            aria-label="View the latest {symbol} {row.kind} file"
                          >
                            <LineChart size={13} strokeWidth={1.75} />
                          </button>
                        {/if}
                        <button
                          class="btn-icon"
                          tabindex="-1"
                          onclick={(e) => { e.stopPropagation(); revealKindDir(row); }}
                          title="Show the output directory"
                          aria-label="Show the output directory for {symbol} {row.kind}"
                        >
                          <FolderOpen size={13} strokeWidth={1.75} />
                        </button>
                      </div>
                    </div>

                    {#if setOpen}
                      <div class="file-rows" role="group">
                        {#if !fs || fs.status === "loading"}
                          <div class="file-note fg-muted text-body-sm">Listing files…</div>
                        {:else if fs.status === "error"}
                          <div class="file-note text-body-sm" style="color: var(--bad)">{fs.message}</div>
                        {:else if fs.files.length === 0}
                          <div class="file-note fg-muted text-body-sm">No files.</div>
                        {:else}
                          {#each fs.files as file (file.path)}
                            {@const fk = fileKey(file)}
                            <div
                              id={rowId(fk)}
                              class="file-row"
                              class:selected={selected.has(fk)}
                              class:cursor={cursor === fk}
                              role="treeitem"
                              aria-level={3}
                              aria-selected={selected.has(fk)}
                              tabindex="-1"
                              title={file.path}
                              onclick={(e) => onRowClick(e, fk)}
                              ondblclick={() => quickLook({ type: "file", key: fk, symbol, row, file })}
                              oncontextmenu={(e) => onContext(e, fk)}
                              onkeydown={() => {}}
                            >
                              <input
                                type="checkbox"
                                class="row-check"
                                aria-label="Select {file.name}"
                                checked={selected.has(fk)}
                                onclick={(e) => onCheck(e, fk)}
                              />
                              <FileText size={13} strokeWidth={1.5} class="file-icon" />
                              <span class="file-date text-figures">
                                {file.start}{#if file.end}<span class="fg-subtle"> → </span>{file.end}{/if}
                              </span>
                              <span class="file-name">{file.name}</span>
                              <span class="file-size text-figures">{fmtBytes(file.bytes)}</span>
                              <span class="file-mod text-figures fg-subtle">{fmtModified(file.modified_ms)}</span>
                            </div>
                          {/each}
                        {/if}
                      </div>
                    {/if}

                    {#if openGaps.has(gapKey(row))}
                      {@const state = gaps[gapKey(row)]}
                      <div class="gap-panel">
                        {#if !state || state.status === "loading"}
                          <span class="gap-note fg-muted text-body-sm">
                            Asking ThetaData which days it has for {row.symbol}…
                          </span>
                        {:else if state.status === "error"}
                          <span class="gap-note text-body-sm" style="color: var(--bad)">
                            {state.message}
                          </span>
                        {:else if state.dates.length === 0}
                          <span class="gap-note fg-muted text-body-sm">
                            Every trading day between {row.first} and {row.last} is on disk.
                          </span>
                        {:else}
                          <div class="gap-head">
                            <span class="gap-note text-body-sm">
                              {fmtNum(state.dates.length)} trading
                              {state.dates.length === 1 ? "day" : "days"} missing between
                              {row.first} and {row.last}. Market holidays are excluded —
                              these are days ThetaData has and you do not.
                            </span>
                            <button
                              class="btn btn-primary btn-sm"
                              disabled={busy}
                              onclick={() => refillGaps(row)}
                            >
                              Queue {fmtNum(state.dates.length)}
                              {state.dates.length === 1 ? "day" : "days"}
                            </button>
                          </div>
                          <ul class="gap-ranges">
                            {#each toRanges(state.dates) as range}
                              <li class="gap-range text-figures">
                                {#if range.days === 1}
                                  {range.from}
                                {:else}
                                  {range.from} → {range.to}
                                  <span class="fg-subtle">({range.days})</span>
                                {/if}
                              </li>
                            {/each}
                          </ul>
                        {/if}
                      </div>
                    {/if}
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {:else if filterQuery}
        <div class="empty-state">
          <p class="empty-label">No results</p>
          <p class="text-body-sm fg-muted">No symbols matching "{filterQuery}".</p>
        </div>
      {/if}

      <!-- ── Available datasets not yet on disk ─────────────────── -->
      {#if !filterQuery && neverDownloaded.length > 0}
        <div class="never-section">
          <button
            type="button"
            class="never-toggle"
            onclick={() => (neverOpen = !neverOpen)}
            aria-expanded={neverOpen}
          >
            {#if neverOpen}
              <ChevronDown size={14} strokeWidth={1.75} />
            {:else}
              <ChevronRight size={14} strokeWidth={1.75} />
            {/if}
            <span class="never-toggle-label">
              Available datasets you don't have on disk
            </span>
            <span class="never-count text-caption tabnum">{neverDownloaded.length}</span>
          </button>

          {#if neverOpen}
            <div class="never-body">
              {#each neverByCategory as group (group.category)}
                <div class="never-category">
                  <div class="never-cat-label text-caption">{group.category}</div>
                  <div class="never-grid">
                    {#each group.entries as entry (entry.name)}
                      <div class="never-card">
                        <div class="never-card-top">
                          <span class="never-card-name">{entry.summary || entry.name}</span>
                          {#if entry.min_tier}
                            <span class="tier-pill tier-{entry.min_tier.toLowerCase()}">{entry.min_tier}</span>
                          {/if}
                        </div>
                        <p class="never-card-desc">{firstSentence(entry.description)}</p>
                        <button
                          type="button"
                          class="never-browse-btn"
                          onclick={() => browseTo(entry.name)}
                          aria-label="Browse {entry.summary}"
                        >
                          Browse
                          <ArrowRight size={12} strokeWidth={1.75} />
                        </button>
                      </div>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    {/if}
  </div>
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

{#if trashItems}
  <ConfirmTrash
    targets={targetsOf(trashItems)}
    names={trashItems.map(itemName)}
    onconfirm={doTrash}
    oncancel={() => { trashItems = null; listEl?.focus(); }}
  />
{/if}

<style>
  .library-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* Header */
  .gap-toggle {
    justify-self: end;
    padding: 2px var(--sp-2);
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    background: transparent;
    color: var(--fg-muted);
    font-size: var(--text-caption);
    font-family: var(--font-ui);
    cursor: pointer;
    white-space: nowrap;
    transition: border-color var(--dur-fast) var(--ease-standard),
                color var(--dur-fast) var(--ease-standard);
  }
  .gap-toggle:hover, .gap-toggle.open {
    border-color: var(--accent);
    color: var(--accent);
  }

  .gap-panel {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    margin: 0 0 var(--sp-2) var(--sp-8);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
  }
  .gap-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-4);
  }
  .gap-note { max-width: 62ch; }
  .gap-ranges {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-3);
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .gap-range {
    font-size: var(--text-caption);
    color: var(--fg-muted);
  }

  .row-msg {
    color: var(--fg-muted);
    margin-left: auto;
    padding-right: var(--sp-3);
    white-space: nowrap;
  }

  .diff-panel {
    border-bottom: 1px solid var(--border);
    padding: 0 var(--space-6) var(--space-4);
  }
  .btn-ghost.active {
    color: var(--accent);
    background: var(--surface-2);
  }
  .lib-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    padding: var(--sp-6) var(--sp-8) var(--sp-4);
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
  }

  .header-left {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .lib-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    color: var(--fg);
    line-height: 1.15;
  }

  .lib-meta {
    font-size: var(--text-body-sm);
    font-variant-numeric: tabular-nums;
  }

  /* Search */
  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  :global(.search-wrap .search-icon) {
    position: absolute;
    left: var(--sp-3);
    color: var(--fg-subtle);
    pointer-events: none;
  }

  .search-input {
    height: 32px;
    padding: 0 var(--sp-3) 0 calc(var(--sp-3) + 14px + var(--sp-2));
    width: 220px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--fg);
    font-size: var(--text-body-sm);
    font-family: var(--font-ui);
    outline: none;
    transition:
      border-color var(--dur-fast) var(--ease-standard),
      box-shadow var(--dur-fast) var(--ease-standard);
  }
  .search-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }
  .search-input::placeholder { color: var(--fg-subtle); }
  .search-input::-webkit-search-cancel-button { display: none; }

  /* Body */
  .lib-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--sp-2) 0;
  }

  .symbol-list { display: flex; flex-direction: column; }

  /* Symbol group */
  .symbol-group {
    border-bottom: 1px solid var(--border);
  }
  .symbol-group:last-child { border-bottom: none; }

  /* The row button and its aside share one line. `.symbol-aside` had
     no rule at all, so it fell below the full-width button and sat
     against the content gutter, reading as a stray control belonging
     to nothing. */
  .symbol-head {
    display: grid;
    grid-template-columns: 18px 20px 80px 1fr auto;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-8) var(--sp-3) var(--sp-6);
    cursor: default;
    user-select: none;
  }
  .symbol-head:hover { background: var(--surface-2); }
  .symbol-aside { flex: none; }

  /* Selection: the accent hue as a soft ramp with an edge, never a slab. */
  .symbol-head.selected,
  .kind-row.selected,
  .file-row.selected {
    background: linear-gradient(90deg, var(--accent-tint-strong), var(--accent-tint) 70%);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .symbol-list:focus-visible .cursor { outline: 1px solid var(--accent); outline-offset: -1px; }
  .symbol-list { outline: none; }

  .row-check {
    width: 14px;
    height: 14px;
    margin: 0;
    accent-color: var(--accent);
    cursor: pointer;
    opacity: 0.45;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }
  .row-check:checked,
  .row-check:indeterminate,
  .symbol-head:hover .row-check,
  .kind-row:hover .row-check,
  .file-row:hover .row-check,
  .list-head .row-check { opacity: 1; }

  .chevron-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    border: none;
    border-radius: var(--r-sm);
    background: none;
    color: var(--fg-subtle);
    cursor: pointer;
  }
  .chevron-btn:hover { color: var(--fg); background: var(--surface-3); }

  .list-head {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-8) var(--sp-2) var(--sp-6);
    border-bottom: 1px solid var(--border);
    color: var(--fg-subtle);
  }
  .list-hint {
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
  }

  .selection-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    padding: var(--sp-2) var(--sp-8);
    border-bottom: 1px solid var(--accent-tint-strong);
    background: linear-gradient(90deg, var(--accent-tint), var(--accent-tint-weak) 60%, transparent);
    flex-shrink: 0;
  }
  .sel-count { font-size: var(--text-body-sm); color: var(--fg-muted); display: inline-flex; gap: 6px; }
  .sel-count strong { color: var(--fg); font-weight: var(--weight-semi); }
  .sel-actions { display: flex; align-items: center; gap: 4px; }
  .sel-trash {
    border: 1px solid var(--bad-ring);
    color: var(--bad);
    background: transparent;
  }
  .sel-trash:hover { background: var(--bad-tint); border-color: var(--bad); }

  .file-rows {
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
    max-height: 420px;
    overflow-y: auto;
  }
  .file-note { padding: var(--sp-2) calc(var(--sp-8) + 64px); }
  .file-row {
    display: grid;
    grid-template-columns: 14px 14px minmax(160px, auto) minmax(0, 1fr) 80px 120px;
    align-items: center;
    gap: var(--sp-3);
    padding: 4px var(--sp-8) 4px calc(var(--sp-6) + 58px);
    font-size: var(--text-body-sm);
    cursor: default;
    user-select: none;
  }
  .file-row:hover { background: var(--surface-3); }
  :global(.file-row .file-icon) { color: var(--fg-subtle); }
  .file-date { color: var(--fg); white-space: nowrap; }
  .file-name {
    font-family: var(--font-mono);
    font-size: var(--text-caption);
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-size { text-align: right; color: var(--fg-muted); }
  .file-mod { text-align: right; white-space: nowrap; }

  .symbol-ticker {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }

  .symbol-summary {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  .sum-stat {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
  }

  .sum-sep { color: var(--fg-subtle); font-size: var(--text-body-sm); }

  /* Kind rows */
  .kind-rows {
    background: var(--surface-1);
    border-top: 1px solid var(--border);
  }

  /* Name and stats share what is left after the gap toggle and the
     actions, and give way before the row overflows: a horizontal
     scrollbar hid the actions entirely. */
  .kind-row {
    display: grid;
    grid-template-columns: minmax(150px, 0.7fr) minmax(0, 1.3fr) auto auto;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-3) var(--sp-8) var(--sp-3) calc(var(--sp-6) + 29px);
    cursor: default;
    user-select: none;
    border-bottom: 1px solid var(--border);
    transition: background var(--dur-fast) var(--ease-standard);
  }
  .kind-row:last-child { border-bottom: none; }
  .kind-row:hover { background: var(--surface-2); }

  .kind-lead {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }
  .kind-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  /* Registry names run long (`option_history_greeks_implied_volatility`)
     and used to overflow into the stats beside them. */
  .kind-name {
    font-family: var(--font-mono);
    font-size: var(--text-figures);
    color: var(--fg-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .kind-title {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .kind-stats {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Dimmed rather than hidden: an action you have to hover to discover
     is one most people never find. */
  .kind-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    opacity: 0.55;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }
  .kind-actions:focus-within { opacity: 1; }
  .kind-row:hover .kind-actions { opacity: 1; }

  /* States */
  .loading-state {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-8);
  }

  .spinner {
    width: 16px; height: 16px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-16);
    text-align: center;
  }
  .empty-icon { opacity: 0.4; }
  .empty-label {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg-muted);
  }

  .empty-cta {
    height: 36px;
    padding: 0 var(--sp-5);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    border-radius: var(--r-md);
    gap: var(--sp-2);
    margin-top: var(--sp-2);
  }

  /* ── Never-downloaded section ──────────────────────────────── */
  .never-section {
    border-top: 1px solid var(--border);
    padding: var(--sp-4) 0;
  }

  .never-toggle {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-8);
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    text-align: left;
    color: var(--fg-muted);
    outline: none;
    transition: color var(--dur-fast) var(--ease-standard);
  }

  .never-toggle:hover { color: var(--fg); }
  .never-toggle:focus-visible { box-shadow: inset var(--shadow-glow-accent); }

  .never-toggle-label {
    flex: 1;
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    color: inherit;
  }

  .never-count {
    color: var(--fg-subtle);
    font-weight: var(--weight-normal);
    text-transform: none;
    letter-spacing: 0;
  }

  .never-body {
    padding: var(--sp-4) var(--sp-8);
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }

  .never-category {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .never-cat-label {
    color: var(--fg-subtle);
    padding-bottom: var(--sp-1);
    border-bottom: 1px solid var(--border);
  }

  .never-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--sp-3);
  }

  .never-card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-4);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    transition: background var(--dur-fast) var(--ease-standard);
  }

  .never-card:hover {
    background: var(--surface-2);
  }

  .never-card-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-2);
  }

  .never-card-name {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
    letter-spacing: -0.005em;
  }

  .never-card-desc {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    line-height: 1.4;
    flex: 1;
    margin: 0;
  }

  .never-browse-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    padding: 3px var(--sp-3);
    background: var(--accent-tint);
    border: 1px solid var(--accent-tint-strong);
    border-radius: var(--r-sm);
    color: var(--accent-hi);
    font-size: var(--text-caption);
    font-weight: var(--weight-semi);
    cursor: pointer;
    align-self: flex-end;
    transition:
      background var(--dur-fast) var(--ease-standard),
      filter var(--dur-fast) var(--ease-standard);
  }

  .never-browse-btn:hover {
    filter: brightness(1.1);
  }

  /* Tier pills in never-downloaded section */
  .tier-pill {
    display: inline-flex;
    padding: 2px 6px;
    border-radius: var(--r-pill);
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
    border: 1px solid transparent;
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .tier-unknown  { background: var(--surface-2);                     color: var(--fg-subtle);       }
</style>
