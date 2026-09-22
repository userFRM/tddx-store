// Stronghold-backed credential persistence. All secrets live in an
// encrypted vault file on disk. The vault password is auto-derived from
// a per-machine salt by the Rust backend so the UI never has to prompt
// for a master password — "remember me" Just Works.
//
// Public API:
//   await vault.save({ email, password })
//   const creds = await vault.load();   // null if no record
//   await vault.clear();
import { Stronghold, Client } from "@tauri-apps/plugin-stronghold";
import { api, TAURI_AVAILABLE } from "$lib/api";

const CLIENT_NAME = "tddx-store";
const KEY_EMAIL = "creds.email";
const KEY_PASSWORD = "creds.password";
const KEY_API_KEY = "creds.api_key";

/** What the user signed in with. Exactly one shape is stored: signing in
 *  one way clears the other, so a stale credential can never be picked
 *  up on the next launch. */
export type StoredCredential =
  | { email: string; password: string; apiKey?: undefined }
  | { apiKey: string; email?: undefined; password?: undefined };

let _strongholdPromise: Promise<{ sh: Stronghold; client: Client }> | null = null;

async function open(): Promise<{ sh: Stronghold; client: Client }> {
  if (!TAURI_AVAILABLE) throw new Error("vault unavailable in browser preview");
  if (_strongholdPromise) return _strongholdPromise;
  _strongholdPromise = (async () => {
    const { vault_path, auto_password } =
      (await api.vaultPaths()) as { vault_path: string; auto_password: string };
    const sh = await Stronghold.load(vault_path, auto_password);
    let client: Client;
    try {
      client = await sh.loadClient(CLIENT_NAME);
    } catch {
      client = await sh.createClient(CLIENT_NAME);
    }
    return { sh, client };
  })().catch((e) => {
    _strongholdPromise = null;
    throw e;
  });
  return _strongholdPromise;
}

const enc = new TextEncoder();
const dec = new TextDecoder();

export const vault = {
  /**
   * Atomic save: insert both fields, persist to disk; on failure roll
   * back the in-memory store changes by removing the (possibly partial)
   * keys. Without this, a sh.save() failure mid-write leaves a torn
   * vault on disk; the next launch reads an inconsistent state.
   */
  async save(creds: StoredCredential): Promise<void> {
    const { sh, client } = await open();
    const store = client.getStore();
    // Snapshot prior values (may be empty) so we can attempt rollback
    // if the persist step fails. Stronghold doesn't expose transactional
    // boundaries, so this is best-effort.
    const prior = {
      [KEY_EMAIL]: await store.get(KEY_EMAIL).catch(() => null),
      [KEY_PASSWORD]: await store.get(KEY_PASSWORD).catch(() => null),
      [KEY_API_KEY]: await store.get(KEY_API_KEY).catch(() => null),
    };
    // Whichever method was not used is removed, so the next launch can
    // never auto-connect with a credential the user replaced.
    const next: Record<string, string | null> =
      creds.apiKey !== undefined
        ? { [KEY_API_KEY]: creds.apiKey, [KEY_EMAIL]: null, [KEY_PASSWORD]: null }
        : { [KEY_EMAIL]: creds.email, [KEY_PASSWORD]: creds.password, [KEY_API_KEY]: null };
    try {
      for (const [key, value] of Object.entries(next)) {
        if (value === null) await store.remove(key).catch(() => {});
        else await store.insert(key, Array.from(enc.encode(value)));
      }
      await sh.save();
    } catch (e) {
      try {
        for (const [key, value] of Object.entries(prior)) {
          if (value) await store.insert(key, Array.from(value));
          else await store.remove(key).catch(() => {});
        }
        await sh.save().catch(() => {});
      } catch {/* rollback best-effort */}
      throw e;
    }
  },

  async load(): Promise<StoredCredential | null> {
    try {
      const { client } = await open();
      const store = client.getStore();
      const k = await store.get(KEY_API_KEY).catch(() => null);
      if (k) return { apiKey: dec.decode(new Uint8Array(k)) };
      const e = await store.get(KEY_EMAIL).catch(() => null);
      const p = await store.get(KEY_PASSWORD).catch(() => null);
      if (!e || !p) return null;
      return {
        email: dec.decode(new Uint8Array(e)),
        password: dec.decode(new Uint8Array(p)),
      };
    } catch {
      return null;
    }
  },

  async clear(): Promise<void> {
    try {
      const { sh, client } = await open();
      const store = client.getStore();
      for (const key of [KEY_EMAIL, KEY_PASSWORD, KEY_API_KEY]) {
        await store.remove(key).catch(() => {});
      }
      await sh.save();
    } catch {
      /* nothing to clear */
    }
  },
};
