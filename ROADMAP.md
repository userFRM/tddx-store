# TdDx Store — Roadmap

> The vision: dataset store + dataset registry + dataset viewer + scheduler
> + support-report builder, all on one ThetaData connection. This file tracks
> what's shipped vs what's queued.

## Status: 2026-05-09

### Shipped tonight

- [x] **Workspace** — Cargo workspace at `~/tdds` with `tdds-core`,
      `tdds-cli`, `apps/desktop` (Tauri 2 + SvelteKit + Vite + TS).
- [x] **Backend engine** — SQLite-backed task queue, async worker pool,
      Arrow → Parquet/CSV/JSON/JSONL writers, on-disk coverage scanner.
- [x] **Registry dispatch** — every one of the 61 thetadatadx endpoints
      reachable via `dispatch_to_file(client, EndpointSpec, …)` and exposed
      through the `endpoints_list` / `endpoint_invoke` / `list_query` Tauri
      commands.
- [x] **Tauri shell** — login gate on launch, remember-me, "Browse /
      Library / Queue / Settings" tabs, persistent right-rail downloads
      pane (Transmission-style), live throughput + ETA, animated progress.
- [x] **Composer** — symbol autocomplete (cached per asset class via
      `stock_list_symbols` / `option_list_roots`), date range slider over
      live `*_list_dates`, multi-symbol bulk input, format radio.
- [x] **Activity console** — in-memory ring buffer (last 1k entries) with
      level filter, "copy ThetaData support report" one-click.

### Sprint 1 — make it production-feeling (1–2 days)

- [ ] **All endpoints surfaced** — generate one card per endpoint from
      `endpoints_list()` so users see all 61, not just the seven curated
      tick kinds. Detail page renders required + optional params from
      registry metadata; one-shot "Run once" button uses `endpoint_invoke`.
- [ ] **Flatfile endpoints** — `flatfile_*` lives outside the registry.
      Add three Tauri commands wrapping `flatfile_option_*` and
      `flatfile_stock_*`. A separate "Flat files" shelf on Browse.
- [ ] **Tauri-plugin-conduit** — drop-in replace `@tauri-apps/api/core`
      `invoke()` with conduit for ~2× IPC throughput on the 1.5 s queue
      polling.
- [ ] **Persisted credentials** — replace in-memory `email/password` with
      a small JSON encrypted by an OS keyring. Today they're settings
      fields that survive only until restart.

### Sprint 2 — viewer + validator (3–5 days)

- [ ] **Data viewer** — preview any local parquet file. New Tauri command
      `parquet_preview(path, offset, limit)` opens the file with
      `parquet::arrow::ArrowReader`, returns N rows as JSON. UI: Detail
      page → Sample tab pulls from this.
- [ ] **Coverage heatmap** — calendar grid over the last N years, cells
      colored by "have / missing / partial" derived from `coverage_report`
      vs `*_list_dates`. Click a missing range → bulk-queue.
- [ ] **Anomaly reviewer** — per-day stats stored alongside each parquet
      (rows, ms_of_day min/max, gap detection). Surface days with
      suspicious ratios (e.g. < 30 % of trailing 90-day average row count)
      as "Needs review" in the Library.
- [ ] **Auto support report** — every error in the activity console gets
      a "Report this" button that builds an HTML email pre-populated with
      activity report + last 10 minutes of relevant log entries + machine
      info, opens via `mailto:support@thetadata.us`.

### Sprint 3 — bulk + scheduling (3–5 days)

- [ ] **Indexkit presets** — depend on `indexkit` crate. Browse shelf
      "Index ecosystems" lists S&P 500 / NDX / Sp400 / Sp600 / DJI / RUT.
      Click → modal: "Queue [stock_trade_quote] for all 503 SP500 symbols
      from 2023-01-01 to 2026-05-09 (parquet)". One click → 503×N tasks.
- [ ] **Multi-symbol watchlists** — saved lists in Settings. Composer
      "From watchlist" dropdown.
- [ ] **Scheduler** — cron-style recurring downloads. New Tauri command
      `schedule_create(spec, cron)` persists in the queue DB. A small
      always-on tokio task in the desktop app fires each schedule. Library
      gets a "Schedules" sub-view: pause / resume / edit / delete.
      Use case: "every weekday 17:30 ET, pull yesterday's flatfile
      `option_trade_quote` for SPX".

### Sprint 4 — polish (ongoing)

- [ ] **Cmd/Ctrl-K palette** — global search across symbols, endpoints,
      saved datasets.
- [ ] **Dataset detail polish** — lazy schema rendering from
      `endpoints_get`, per-row description + example.
- [ ] **Web mode** — small axum bridge serving the same UI over HTTP for
      Tailscale-shared mobile / Mac access without a desktop install.
- [ ] **Telegram / Slack notifications** on failed schedules.

## Backlog (parking lot)

- Saved searches with cmd-shift-S
- Diff between two coverage snapshots ("show me what changed since
  yesterday")
- Export of activity report as JSON for programmatic ingestion
- "Estimate cost" widget — given symbol × date range × kind, predict
  byte size from per-day historical density
- Rate-limit observer panel (visualises the gRPC request_semaphore
  permits in flight)
