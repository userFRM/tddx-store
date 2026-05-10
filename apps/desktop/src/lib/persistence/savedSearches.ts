// Saved searches / watchlists. Persisted via tauri-plugin-store (kv.ts
// wrapper) so the file lives in the OS-correct app data dir resolved
// by `app.path().app_data_dir()` server-side. Falls back to
// localStorage in vite preview where the Tauri runtime is absent.
//
// A "search" is a partial composer config: kind + symbols + date range +
// format. Cmd-K can preview them, Browse can pin them, the composer can
// load them into the form for one-click re-run.

import { app } from "$lib/stores/app.svelte";
import { kvGet, kvSet } from "$lib/persistence/kv";

const STORAGE_KEY = "tdds.saved_searches.v1";

export interface SavedSearch {
  id: string;
  name: string;
  kind: string;
  symbols: string[];      // 1+ tickers
  start: string | null;   // YYYY-MM-DD
  end: string | null;
  format: string;
  createdAt: number;
  lastUsedAt: number | null;
}

async function loadFromStore(): Promise<SavedSearch[]> {
  const v = await kvGet<SavedSearch[]>(STORAGE_KEY);
  return Array.isArray(v) ? v : [];
}

function persist(list: SavedSearch[]) {
  // Fire-and-forget; saving is best-effort and the UI already has the
  // in-memory copy. Errors get logged via the kv.ts plugin path.
  void kvSet(STORAGE_KEY, list);
}

/** Hydrate the in-memory mirror from tauri-plugin-store. Call once on mount. */
export async function loadSavedSearches() {
  app.savedSearches = await loadFromStore();
}

export function listSavedSearches(): SavedSearch[] {
  return app.savedSearches.slice().sort((a, b) => {
    // Most-recently-used first; new ones (no lastUsedAt) at the end of
    // the freshly-saved cluster.
    const ax = a.lastUsedAt ?? a.createdAt;
    const bx = b.lastUsedAt ?? b.createdAt;
    return bx - ax;
  });
}

export function saveSearch(input: Omit<SavedSearch, "id" | "createdAt" | "lastUsedAt">): SavedSearch {
  const s: SavedSearch = {
    id: crypto.randomUUID(),
    createdAt: Date.now(),
    lastUsedAt: null,
    ...input,
  };
  app.savedSearches = [s, ...app.savedSearches];
  persist(app.savedSearches);
  return s;
}

export function deleteSavedSearch(id: string) {
  app.savedSearches = app.savedSearches.filter((s) => s.id !== id);
  persist(app.savedSearches);
}

export function touchSavedSearch(id: string) {
  app.savedSearches = app.savedSearches.map((s) =>
    s.id === id ? { ...s, lastUsedAt: Date.now() } : s,
  );
  persist(app.savedSearches);
}

export function renameSavedSearch(id: string, name: string) {
  app.savedSearches = app.savedSearches.map((s) =>
    s.id === id ? { ...s, name } : s,
  );
  persist(app.savedSearches);
}
