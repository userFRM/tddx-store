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
  async save(creds: { email: string; password: string }): Promise<void> {
    const { sh, client } = await open();
    const store = client.getStore();
    // Snapshot prior values (may be empty) so we can attempt rollback
    // if the persist step fails. Stronghold doesn't expose transactional
    // boundaries, so this is best-effort.
    const priorEmail    = await store.get(KEY_EMAIL).catch(() => null);
    const priorPassword = await store.get(KEY_PASSWORD).catch(() => null);
    try {
      await store.insert(KEY_EMAIL,    Array.from(enc.encode(creds.email)));
      await store.insert(KEY_PASSWORD, Array.from(enc.encode(creds.password)));
      await sh.save();
    } catch (e) {
      try {
        if (priorEmail)    await store.insert(KEY_EMAIL,    Array.from(priorEmail));
        else                await store.remove(KEY_EMAIL).catch(() => {});
        if (priorPassword) await store.insert(KEY_PASSWORD, Array.from(priorPassword));
        else                await store.remove(KEY_PASSWORD).catch(() => {});
        await sh.save().catch(() => {});
      } catch {/* rollback best-effort */}
      throw e;
    }
  },

  async load(): Promise<{ email: string; password: string } | null> {
    try {
      const { client } = await open();
      const store = client.getStore();
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
      await store.remove(KEY_EMAIL).catch(() => {});
      await store.remove(KEY_PASSWORD).catch(() => {});
      await sh.save();
    } catch {
      /* nothing to clear */
    }
  },
};
