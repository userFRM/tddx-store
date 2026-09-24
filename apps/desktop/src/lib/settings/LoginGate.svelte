<script lang="ts">
  /**
   * Full-screen login overlay shown until the user is `connected`.
   * A credential saved on this device (encrypted Stronghold vault, opt-in
   * via "Remember me") prefills the form; signing in is always the
   * user's click. The overlay disappears as soon as
   * `app.connState === "connected"`.
   */
  import { onMount } from "svelte";
  import {
    Mail,
    Lock,
    LogIn,
    Eye,
    EyeOff,
    AlertCircle,
    Loader2,
    KeyRound,
  } from "lucide-svelte";
  import {
    app,
    startQueuePoll,
    loadSettings,
    warmCaches,
    refreshTierStatus,
    log,
  } from "$lib/stores/app.svelte";
  import ThetaDataLogo from "$lib/brand/ThetaDataLogo.svelte";
  import { api, TAURI_AVAILABLE } from "$lib/api";
  import { vault, type StoredCredential } from "$lib/persistence/vault";

  /** ThetaData accepts either credential. A key is revocable from the
   *  account portal without changing the password, so it is the better
   *  one to leave sitting in a downloader. */
  type Method = "password" | "api_key";

  let method = $state<Method>("password");
  let email = $state("");
  let password = $state("");
  let apiKey = $state("");
  /** Optional with a key. Market data needs only the key; the flat-file
   *  server also wants the account email, and rejects a key-only
   *  sign-in. */
  let keyEmail = $state("");
  let remember = $state(true);
  let showSecret = $state(false);
  let signingIn = $state(false);

  const ready = $derived(
    method === "password" ? Boolean(email && password) : Boolean(apiKey),
  );

  /** The credential saved on this device, if any. It fills the form
   *  rather than signing in on its own: a launch that connects before
   *  the user has done anything is indistinguishable from someone
   *  else's session, and the user should see which account is used. */
  let saved = $state<StoredCredential | null>(null);

  /** Last four characters, enough to tell two keys apart. */
  const savedLabel = $derived.by(() => {
    if (!saved) return "";
    if (saved.apiKey) {
      const tail = saved.apiKey.slice(-4);
      return `API key ••••${tail}${saved.email ? ` · ${saved.email}` : ""}`;
    }
    return saved.email ?? "";
  });

  async function loadSaved() {
    saved = await vault.load().catch(() => null);
    if (saved?.apiKey) {
      method = "api_key";
      apiKey = saved.apiKey;
      keyEmail = saved.email ?? "";
    } else if (saved?.password) {
      method = "password";
      email = saved.email ?? "";
      password = saved.password;
    }
  }

  async function forgetSaved() {
    try {
      await vault.clear();
      log("info", "Saved credential removed from this device");
    } catch (e: unknown) {
      log("error", `Couldn't remove the saved credential: ${e instanceof Error ? e.message : String(e)}`);
      return;
    }
    saved = null;
    apiKey = "";
    keyEmail = "";
    email = "";
    password = "";
  }

  onMount(async () => {
    await loadSettings();
    await loadSaved();
  });

  // Coming back to the gate after "Sign out": the vault was just
  // cleared, so start from an empty form instead of the secret that was
  // in it a moment ago.
  let wasConnected = false;
  $effect(() => {
    const connected = app.connState === "connected";
    if (wasConnected && app.connState === "idle") {
      saved = null;
      apiKey = "";
      keyEmail = "";
      email = "";
      password = "";
    }
    wasConnected = connected;
  });

  async function trySignIn() {
    if (!ready) {
      app.connState = "error";
      app.connMsg =
        method === "password" ? "Email and password required" : "API key required";
      return;
    }
    signingIn = true;
    app.connState = "connecting";
    app.connMsg = "Signing in…";
    try {
      if (method === "password") {
        app.settings.email = email;
        app.settings.password = remember ? password : "";
        await api.settingsSet(app.settings);
        await api.login({ method: "password", email, password });
      } else {
        await api.login({ method: "api_key", api_key: apiKey, email: keyEmail.trim() || null });
      }
      app.connState = "connected";
      app.connMsg = method === "password" ? `Signed in as ${email}` : "Signed in with API key";
      log("info", app.connMsg);

      // Everything the UI needs is already in hand: the tiers came back
      // with the auth response. Fire these before touching the vault,
      // which opens and re-encrypts an on-disk snapshot and took long
      // enough to leave the tier badges blank for seconds.
      startQueuePoll();
      warmCaches();
      refreshTierStatus();

      // Persist (encrypted) on opt-in, without blocking the UI on it.
      const credential =
        method === "password"
          ? { email, password }
          : { apiKey, email: keyEmail.trim() || undefined };
      void (remember
        ? vault.save(credential).catch((e) => log("warn", `vault save failed: ${e}`))
        : vault.clear().catch(() => {}));

      // Don't keep the secret in component state once connected.
      password = "";
      apiKey = "";
    } catch (e: unknown) {
      app.connState = "error";
      app.connMsg = e instanceof Error ? e.message : String(e);
      log("error", `Login failed: ${app.connMsg}`);
    } finally {
      signingIn = false;
    }
  }

  function onSubmit(e: SubmitEvent) {
    e.preventDefault();
    void trySignIn();
  }

  // Show the gate until we're connected. While connecting (auto path),
  // keep showing it but with a loader so the UI never flashes empty.
  const showGate = $derived(app.connState !== "connected");
</script>

{#if showGate}
  <div class="gate-backdrop">
    <div class="gate">
      <div class="brand-row">
        <span class="brand-logo"><ThetaDataLogo height={34} suffix="Store" /></span>
      </div>
      <h1 class="gate-title">Sign in to ThetaData</h1>
      <p class="gate-sub">Streams market data using your ThetaData account.</p>

      <form class="gate-form" onsubmit={onSubmit}>
        {#if saved}
          <div class="saved-row">
            <KeyRound size={14} />
            <span class="saved-text">
              Saved on this device: <strong class="text-figures">{savedLabel}</strong>
            </span>
            <button type="button" class="link-btn" onclick={forgetSaved} disabled={signingIn}>
              Forget
            </button>
          </div>
        {/if}
        <div class="method-switch" role="radiogroup" aria-label="Sign-in method">
          <button
            type="button"
            role="radio"
            aria-checked={method === "password"}
            class:active={method === "password"}
            onclick={() => (method = "password")}
            disabled={signingIn}
          >
            Email and password
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={method === "api_key"}
            class:active={method === "api_key"}
            onclick={() => (method = "api_key")}
            disabled={signingIn}
          >
            API key
          </button>
        </div>

        {#if method === "password"}
          <label class="field-stack">
            <span class="text-caption">Email</span>
            <div class="input-with-icon">
              <Mail size={14} class="input-icon" />
              <input
                class="field-input padded"
                type="email"
                autocomplete="username"
                placeholder="you@example.com"
                bind:value={email}
                disabled={signingIn}
              />
            </div>
          </label>

          <label class="field-stack">
            <span class="text-caption">Password</span>
            <div class="input-with-icon">
              <Lock size={14} class="input-icon" />
              <input
                class="field-input padded with-trailing"
                type={showSecret ? "text" : "password"}
                autocomplete="current-password"
                placeholder="••••••••••••"
                bind:value={password}
                disabled={signingIn}
              />
              <button
                type="button"
                class="trailing-btn"
                aria-label={showSecret ? "Hide password" : "Show password"}
                onclick={() => (showSecret = !showSecret)}
              >
                {#if showSecret}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
              </button>
            </div>
          </label>
        {:else}
          <label class="field-stack">
            <span class="text-caption">API key</span>
            <div class="input-with-icon">
              <KeyRound size={14} class="input-icon" />
              <input
                class="field-input padded with-trailing text-figures"
                type={showSecret ? "text" : "password"}
                autocomplete="off"
                spellcheck="false"
                placeholder="Paste the key from your account portal"
                bind:value={apiKey}
                disabled={signingIn}
              />
              <button
                type="button"
                class="trailing-btn"
                aria-label={showSecret ? "Hide API key" : "Show API key"}
                onclick={() => (showSecret = !showSecret)}
              >
                {#if showSecret}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
              </button>
            </div>
            <span class="field-hint text-body-sm fg-muted">
              Generate one in the ThetaData account portal. Setting
              <code>THETADATA_API_KEY</code> in the environment signs you in
              without typing it here.
            </span>
          </label>
          <label class="field-stack">
            <span class="text-caption">Account email <span class="fg-subtle">· optional</span></span>
            <input
              class="field-input"
              type="email"
              autocomplete="email"
              placeholder="you@example.com"
              bind:value={keyEmail}
              disabled={signingIn}
            />
            <span class="field-hint text-body-sm fg-muted">
              Needed only for whole-market flat files — their server wants the
              email alongside the key. Everything else works without it.
            </span>
          </label>
        {/if}

        <label class="remember">
          <input type="checkbox" bind:checked={remember} disabled={signingIn} />
          <span class="text-body-sm">Remember me on this device</span>
        </label>

        {#if app.connState === "error"}
          <div class="gate-error">
            <AlertCircle size={14} />
            <span>{app.connMsg}</span>
          </div>
        {/if}

        {#if !TAURI_AVAILABLE}
          <div class="gate-warn">
            <AlertCircle size={14} />
            <span>
              Browser preview mode — login disabled. Run the desktop build
              (<code>npm run tauri dev</code>) to sign in.
            </span>
          </div>
        {/if}

        <button
          type="submit"
          class="btn btn-primary gate-btn"
          disabled={signingIn || !ready || !TAURI_AVAILABLE}
        >
          {#if signingIn}
            <Loader2 class="spin" size={14} />
            {app.connMsg}
          {:else}
            <LogIn size={14} />
            Sign in
          {/if}
        </button>

        <p class="gate-hint">
          Don't have an account? <a href="https://thetadata.net/pricing" target="_blank" rel="noreferrer">Sign up at thetadata.net</a>.
        </p>
      </form>
    </div>
  </div>
{/if}

<style>
  .gate-backdrop {
    position: fixed;
    inset: 0;
    background: var(--bg);
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--sp-8);
  }
  .gate {
    width: 100%;
    max-width: 420px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    padding: var(--sp-8);
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    box-shadow: var(--shadow-modal);
  }
  .brand-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-bottom: var(--sp-2);
  }
  .method-switch {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
    padding: 2px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
  }
  .method-switch button {
    padding: 6px var(--sp-2);
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-standard),
                color var(--dur-fast) var(--ease-standard);
  }
  .method-switch button:hover:not(.active) { color: var(--fg); }
  .method-switch button.active {
    background: var(--surface-1);
    color: var(--fg);
    box-shadow: var(--shadow-flat);
  }
  .field-hint code {
    font-family: var(--font-mono);
    font-size: var(--text-caption);
  }

  .brand-logo {
    display: block;
    -webkit-user-select: none;
    user-select: none;
  }
  .gate-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    margin: 0;
  }
  .gate-sub {
    color: var(--fg-muted);
    margin: 0 0 var(--sp-3);
  }
  .gate-form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .field-stack { display: flex; flex-direction: column; gap: 6px; }
  .input-with-icon { position: relative; display: flex; align-items: center; }
  .input-with-icon :global(.input-icon) {
    position: absolute; left: var(--sp-3);
    color: var(--fg-subtle); pointer-events: none;
  }
  .field-input.padded { padding-left: 32px; height: 36px; }
  .field-input.with-trailing { padding-right: 36px; }
  .trailing-btn {
    position: absolute; right: 6px; top: 50%; transform: translateY(-50%);
    background: transparent; border: 0; color: var(--fg-subtle);
    cursor: pointer; padding: 4px; display: inline-flex;
  }
  .trailing-btn:hover { color: var(--fg); }

  .saved-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 8px 10px;
    border: 1px solid var(--accent-tint-strong);
    border-radius: var(--r-md);
    background: linear-gradient(135deg, var(--accent-tint), var(--accent-tint-weak) 60%, transparent);
    color: var(--accent);
    font-size: var(--text-body-sm);
  }
  .saved-text { flex: 1; min-width: 0; color: var(--fg); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .saved-text strong { font-weight: var(--weight-semi); }
  .link-btn {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
    text-decoration: underline;
  }
  .remember {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--fg-muted);
    cursor: pointer;
  }
  .remember input { accent-color: var(--accent); }

  .gate-error, .gate-warn {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-sm);
    font-size: var(--text-body-sm);
  }
  .gate-error {
    color: var(--bad);
    background: var(--bad-tint);
    border: 1px solid var(--bad-tint);
  }
  .gate-warn {
    color: var(--warn);
    background: var(--warn-tint);
    border: 1px solid var(--warn-tint);
  }
  .gate-warn code {
    font-family: var(--font-mono);
    background: var(--surface-3);
    padding: 0 4px;
    border-radius: 3px;
  }

  .gate-btn {
    height: 38px;
    justify-content: center;
    margin-top: var(--sp-1);
  }
  .gate-hint {
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    margin: 0;
    text-align: center;
  }
  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
