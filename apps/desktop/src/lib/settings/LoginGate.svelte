<script lang="ts">
  /**
   * Full-screen login overlay shown until the user is `connected`.
   * Reads/writes saved credentials via the backend (memory + opt-in
   * persisted JSON next to the queue DB). The overlay disappears as
   * soon as `app.connState === "connected"`.
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
  } from "lucide-svelte";
  import {
    app,
    startQueuePoll,
    loadSettings,
    warmCaches,
    refreshTierStatus,
    log,
  } from "$lib/stores/app.svelte";
  import { api, TAURI_AVAILABLE } from "$lib/api";
  import { vault } from "$lib/persistence/vault";

  let email = $state("");
  let password = $state("");
  let remember = $state(true);
  let showPw = $state(false);
  let signingIn = $state(false);

  onMount(async () => {
    await loadSettings();
    // Try to load creds from the encrypted Stronghold vault first; fall
    // back to in-memory settings (rarely populated unless the user just
    // typed them and we crashed before vault save completed).
    const stored = await vault.load().catch(() => null);
    if (stored) {
      email = stored.email;
      password = stored.password;
      remember = true;
      await trySignIn(/* fromAuto */ true);
    } else if (app.settings.email && app.settings.password) {
      email = app.settings.email;
      password = app.settings.password;
      remember = true;
      await trySignIn(true);
    } else if (app.settings.email) {
      email = app.settings.email;
    }
  });

  async function trySignIn(fromAuto = false) {
    if (!email || !password) {
      app.connState = "error";
      app.connMsg = "Email and password required";
      return;
    }
    signingIn = true;
    app.connState = "connecting";
    app.connMsg = fromAuto ? "Auto-signing in…" : "Signing in…";
    try {
      app.settings.email = email;
      app.settings.password = remember ? password : "";
      await api.settingsSet(app.settings);
      await api.login({ email, password });
      app.connState = "connected";
      app.connMsg = `Signed in as ${email}`;
      log("info", `Signed in as ${email}`);
      // Persist (encrypted) on opt-in.
      if (remember) {
        await vault.save({ email, password }).catch((e) =>
          log("warn", `vault save failed: ${e}`),
        );
      } else {
        await vault.clear().catch(() => {});
      }
      startQueuePoll();
      warmCaches();           // pre-load symbol / root lists for autocomplete
      // Populate per-asset-class tier badges (Home + topbar) — login
      // is the only handshake where the SDK knows the user's tiers.
      refreshTierStatus();
      // Don't keep the password in component state once connected.
      password = "";
    } catch (e: unknown) {
      app.connState = "error";
      app.connMsg = e instanceof Error ? e.message : String(e);
      log("error", `Login failed: ${app.connMsg}`);
    } finally {
      signingIn = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") trySignIn();
  }

  // Show the gate until we're connected. While connecting (auto path),
  // keep showing it but with a loader so the UI never flashes empty.
  const showGate = $derived(app.connState !== "connected");
</script>

{#if showGate}
  <div class="gate-backdrop">
    <div class="gate">
      <div class="brand-row">
        <img
          class="brand-logo"
          src="/thetadata-logo.svg"
          alt="ThetaData"
          draggable="false"
        />
        <span class="brand-name">Store</span>
      </div>
      <h1 class="gate-title">Sign in to ThetaData</h1>
      <p class="gate-sub">Streams market data using your ThetaData account.</p>

      <div class="gate-form" role="group" onkeydown={onKey}>
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
              type={showPw ? "text" : "password"}
              autocomplete="current-password"
              placeholder="••••••••••••"
              bind:value={password}
              disabled={signingIn}
            />
            <button
              type="button"
              class="trailing-btn"
              aria-label={showPw ? "Hide password" : "Show password"}
              onclick={() => (showPw = !showPw)}
            >
              {#if showPw}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
            </button>
          </div>
        </label>

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
          class="btn btn-primary gate-btn"
          onclick={() => trySignIn()}
          disabled={signingIn || !email || !password || !TAURI_AVAILABLE}
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
      </div>
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
  .brand-logo {
    height: 28px;
    width: auto;
    display: block;
    -webkit-user-select: none;
    user-select: none;
  }
  .brand-name {
    font-weight: var(--weight-semi);
    font-size: 17px;
    color: var(--fg-muted);
    margin-left: 2px;
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
    background: rgba(255, 126, 126, 0.08);
    border: 1px solid rgba(255, 126, 126, 0.25);
  }
  .gate-warn {
    color: var(--warn);
    background: rgba(245, 197, 111, 0.08);
    border: 1px solid rgba(245, 197, 111, 0.25);
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
