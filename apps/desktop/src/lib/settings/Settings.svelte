<script lang="ts">
  /**
   * Settings = Login + Storage. The credentials section is a real login
   * form (email + password), no creds-file path. Once authenticated the
   * email is shown alongside a "Sign out" affordance.
   */
  import { onMount } from "svelte";
  import PreferencesCard from "$lib/settings/PreferencesCard.svelte";
  import {
    Save,
    LogIn,
    LogOut,
    CheckCircle2,
    AlertCircle,
    Eye,
    EyeOff,
    Mail,
    Lock,
    ArrowUpRight,
  } from "lucide-svelte";
  import { app, startQueuePoll, stopQueuePoll, loadSettings, log, navigate } from "$lib/stores/app.svelte";
  import { api } from "$lib/api";
  import { vault } from "$lib/persistence/vault";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let email = $state("");
  let password = $state("");
  let showPw = $state(false);
  let signingIn = $state(false);
  let saveMsg = $state("");

  // No tier/concurrency math on the FE — `tier_status` already
  // normalizes Unknown→Free and computes the account-wide budget.
  // We just render.

  async function handleUpgrade() {
    const url = app.tierStatus?.upgrade_url;
    if (!url) return;
    try { await openUrl(url); } catch {}
  }

  /** One-line per-class summary for the Account card: e.g.
   *  `Stocks Standard · Options Pro · Indices Free · Rates Free · 8 concurrent`. */
  const accountSummary = $derived.by<string>(() => {
    const t = app.tierStatus;
    if (!t) return "Connected to ThetaData";
    const parts = t.classes.map((c) => `${c.label} ${c.tier}`);
    return `${parts.join(" · ")} · ${t.in_flight_budget} concurrent`;
  });

  onMount(async () => {
    await loadSettings();
    // The backend never echoes the password back; only restore the email.
    email = app.settings.email ?? "";
  });

  async function signIn() {
    if (!email || !password) {
      app.connState = "error";
      app.connMsg = "Email and password required";
      return;
    }
    signingIn = true;
    app.connState = "connecting";
    app.connMsg = "Signing in…";
    try {
      app.settings.email = email;
      app.settings.password = password;
      await api.settingsSet(app.settings);
      await api.login({ method: "password", email, password });
      app.connState = "connected";
      app.connMsg = `Signed in as ${email}`;
      startQueuePoll();
      // Don't keep plaintext password in $state once we're in.
      password = "";
    } catch (e: unknown) {
      app.connState = "error";
      app.connMsg = e instanceof Error ? e.message : String(e);
    } finally {
      signingIn = false;
    }
  }

  async function signOut() {
    // The UI answers the click at once: the sign-in screen comes up
    // before the teardown, which opens and re-encrypts the vault and
    // can take seconds. Waiting on it first is what made the button
    // look dead.
    stopQueuePoll();
    // The next sign-in starts a fresh session, so it opens on Home
    // rather than the Settings page the previous one ended on.
    navigate("home");
    app.connState = "idle";
    app.connMsg = "";
    app.tierStatus = null;
    app.settings.email = "";
    app.settings.password = "";
    email = "";
    password = "";

    // Then the backend: drops the live client, aborts the worker pool,
    // clears in-memory credentials, and removes the saved credential so
    // the next launch doesn't sign straight back in.
    try {
      await api.logout();
    } catch (e: unknown) {
      log("error", `Sign-out: backend teardown failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    try {
      await vault.clear();
      log("info", "Signed out; saved credential removed");
    } catch (e: unknown) {
      // Said on the sign-in screen, which is what is showing now; a toast
      // would sit underneath it.
      const msg = `Signed out, but couldn't remove the saved credential: ${e instanceof Error ? e.message : String(e)}`;
      app.connState = "error";
      app.connMsg = msg;
      log("warn", msg);
    }
  }

  async function saveStorage() {
    try {
      await api.settingsSet(app.settings);
      saveMsg = "Saved";
      setTimeout(() => (saveMsg = ""), 1500);
    } catch (e: unknown) {
      saveMsg = e instanceof Error ? e.message : String(e);
    }
  }

  function onLoginSubmit(e: SubmitEvent) {
    e.preventDefault();
    void signIn();
  }
</script>

<div class="settings-view">
  <header class="settings-header">
    <span class="text-caption">Settings</span>
    <h1 class="settings-title">Account &amp; storage</h1>
    <p class="settings-sub fg-muted">
      Sign in with your ThetaData credentials and tell the app where to
      store the queue and the parquet files it writes.
    </p>
  </header>

  <div class="settings-grid">
    <!-- ── Account card ────────────────────────────────────── -->
    <section class="settings-card">
      <h2 class="card-heading">Account</h2>

      {#if app.connState === "connected"}
        <div class="signed-in">
          <div class="signed-in-icon">
            <CheckCircle2 size={20} />
          </div>
          <div class="signed-in-body">
            <div class="signed-in-email">{app.settings.email || email}</div>
            <div class="signed-in-status text-body-sm fg-muted">
              {accountSummary}
            </div>
          </div>
          <button class="btn btn-ghost" onclick={signOut}>
            <LogOut size={14} /> Sign out
          </button>
        </div>
      {:else}
        <form class="login-form" onsubmit={onLoginSubmit}>
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

          {#if app.connState === "error"}
            <div class="login-error">
              <AlertCircle size={14} />
              <span>{app.connMsg}</span>
            </div>
          {/if}

          <button
            type="submit"
            class="btn btn-primary login-btn"
            disabled={signingIn || !email || !password}
          >
            <LogIn size={14} />
            {signingIn ? "Signing in…" : "Sign in"}
          </button>

          <p class="login-hint fg-muted text-body-sm">
            ThetaData credentials. Stored in app memory only — never written
            to disk by the app.
          </p>
        </form>
      {/if}
    </section>

    <!-- ── Storage card ────────────────────────────────────── -->
    <section class="settings-card">
      <h2 class="card-heading">Storage</h2>
      <label class="field-stack">
        <span class="text-caption">Queue database</span>
        <input class="field-input" bind:value={app.settings.db_path}
               placeholder="$HOME/tddx-store/queue.db" />
        <span class="hint fg-muted">SQLite file. Survives restarts; stores task list.</span>
      </label>
      <label class="field-stack">
        <span class="text-caption">Output directory</span>
        <input class="field-input" bind:value={app.settings.output_dir}
               placeholder="$HOME/tddx-store/data" />
        <span class="hint fg-muted">One subdirectory per kind; one file per (symbol, date).</span>
      </label>
      <div class="field-stack">
        <span class="text-caption">Subscription tier per asset class</span>
        <div class="conc-grid">
          {#each app.tierStatus?.classes ?? [] as row (row.class)}
            <div class="conc-row">
              <span class="conc-label">{row.label}</span>
              <span class="conc-tier fg-muted">{row.tier}</span>
              {#if !row.at_max}
                <button
                  type="button"
                  class="btn btn-primary btn-sm"
                  onclick={handleUpgrade}
                  aria-label={`Upgrade ${row.label.toLowerCase()} tier`}
                >
                  <ArrowUpRight size={10} strokeWidth={2} />
                  Upgrade
                </button>
              {:else}
                <span aria-hidden="true"></span>
              {/if}
            </div>
          {/each}
        </div>
        <span class="hint fg-muted">
          {app.tierStatus?.in_flight_budget ?? 1} downloads run at once.
          ThetaData sizes that budget per account, not per asset class,
          so it follows your highest tier. Free tier is granted to every
          account by default; upgrades take effect on the next queue run.
        </span>
      </div>
    </section>
  </div>

  <PreferencesCard />

  <footer class="settings-footer">
    <span class="hint fg-muted">{saveMsg}</span>
    <button class="btn btn-secondary" onclick={saveStorage}>
      <Save size={14} /> Save settings
    </button>
  </footer>
</div>

<style>
  .settings-view {
    padding: var(--sp-8);
    overflow-y: auto;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
    max-width: 920px;
  }

  .settings-header { display: flex; flex-direction: column; gap: var(--sp-1); }
  .settings-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    color: var(--fg);
    margin: 0;
  }
  .settings-sub { font-size: var(--text-body); margin: 0; max-width: 560px; }

  .settings-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-4);
    /* Two cards of very different height sit side by side — Account is
       a status line and a button, Storage is two paths and a tier
       table. Stretching Account to match left a card that was mostly
       empty. */
    align-items: start;
  }
  @media (max-width: 1100px) { .settings-grid { grid-template-columns: 1fr; } }

  .settings-card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: var(--sp-5);
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .card-heading {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg);
    margin: 0;
  }
  .field-stack { display: flex; flex-direction: column; gap: 6px; }
  .hint, .login-hint { font-size: var(--text-body-sm); }

  .input-with-icon { position: relative; display: flex; align-items: center; }
  .input-with-icon :global(.input-icon) {
    position: absolute;
    left: var(--sp-3);
    color: var(--fg-subtle);
    pointer-events: none;
  }
  .field-input.padded { padding-left: 32px; }
  .field-input.with-trailing { padding-right: 36px; }
  .trailing-btn {
    position: absolute;
    right: 6px;
    top: 50%;
    transform: translateY(-50%);
    background: transparent;
    border: 0;
    color: var(--fg-subtle);
    cursor: pointer;
    padding: 4px;
    display: inline-flex;
  }
  .trailing-btn:hover { color: var(--fg); }

  .login-form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .login-error {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--bad);
    font-size: var(--text-body-sm);
    background: var(--bad-tint);
    border: 1px solid var(--bad-tint);
    border-radius: var(--r-sm);
    padding: var(--sp-2) var(--sp-3);
  }
  .login-btn {
    align-self: stretch;
    height: 36px;
    justify-content: center;
  }

  .signed-in {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .signed-in-icon {
    width: 36px;
    height: 36px;
    border-radius: var(--r-sm);
    background: var(--good-tint);
    color: var(--good);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .signed-in-body { flex: 1; min-width: 0; }
  .signed-in-email {
    font-weight: var(--weight-semi);
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .signed-in-status { margin-top: 2px; }

  .settings-footer {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: var(--sp-3);
  }

  .conc-grid {
    display: grid;
    grid-template-columns: 1fr;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    overflow: hidden;
    background: var(--surface-2);
  }
  .conc-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: var(--sp-3);
    align-items: center;
    padding: 8px var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .conc-row:first-child { border-top: 0; }
  .conc-label { color: var(--fg); font-size: var(--text-body-sm); }
  .conc-tier {
    font-size: var(--text-caption);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    min-width: 5em;
    text-align: right;
  }
</style>
