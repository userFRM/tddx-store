/*
 * Persistent key/value backed by tauri-plugin-store.
 *
 * The plugin persists a single JSON file in the OS-correct app data
 * dir (macOS Application Support, Linux XDG, Windows %APPDATA%) — same
 * directory the Rust side resolves via `app.path().app_data_dir()`.
 * That makes the storage reachable from BOTH sides without duplicating
 * the path-derivation logic, and survives uninstall-style cleanups
 * because it lives under the bundle identifier.
 *
 * In a plain browser preview (vite preview without Tauri) the plugin
 * is unavailable; we fall back to `localStorage` so the UI renders the
 * same. Outside of preview, every read and write goes through Tauri.
 */
import { load, type Store } from "@tauri-apps/plugin-store";

const FILE = "tdds-store.json";

const isTauri =
  typeof window !== "undefined" &&
  (("__TAURI_INTERNALS__" in window) || ("__TAURI__" in window));

let _store: Store | null = null;
let _initPromise: Promise<Store | null> | null = null;

async function store(): Promise<Store | null> {
  if (!isTauri) return null;
  if (_store) return _store;
  if (!_initPromise) {
    _initPromise = load(FILE, { autoSave: true, defaults: {} }).then((s) => {
      _store = s;
      return s;
    });
  }
  return _initPromise;
}

export async function kvGet<T = unknown>(key: string): Promise<T | undefined> {
  const s = await store();
  if (s) return (await s.get<T>(key)) ?? undefined;
  if (typeof localStorage === "undefined") return undefined;
  const raw = localStorage.getItem(key);
  if (raw == null) return undefined;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return raw as unknown as T;
  }
}

export async function kvSet<T>(key: string, value: T): Promise<void> {
  const s = await store();
  if (s) {
    await s.set(key, value);
    return;
  }
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(key, JSON.stringify(value));
}

export async function kvRemove(key: string): Promise<void> {
  const s = await store();
  if (s) {
    await s.delete(key);
    return;
  }
  if (typeof localStorage === "undefined") return;
  localStorage.removeItem(key);
}

export async function kvHas(key: string): Promise<boolean> {
  const s = await store();
  if (s) return s.has(key);
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem(key) != null;
}
