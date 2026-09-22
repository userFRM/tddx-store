# Changelog

All notable changes to TdDx Store are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- **The app now builds.** It depended on `thetadatadx` and `tdbe` at git
  tag `v10.0.0`; the SDK reset its version line and that tag no longer
  exists, so nothing in the workspace could resolve its dependencies.
  It now takes `thetadatadx-rs` 0.4 from crates.io, where the former
  `tdbe` crate lives as a module. Arrow moved 58 → 59 to match the SDK,
  which otherwise puts two incompatible `RecordBatch` types in the
  graph.
- **Queueing a dataset from Browse.** The catalogue names datasets by
  their registry endpoint while the queue accepted seven legacy names,
  so every dataset outside that overlap failed with `unknown kind`.
  Both sides now speak the registry's vocabulary; the legacy names
  still resolve so existing queue rows, schedules and output
  directories keep working.
- **Parquet column sets.** Tick batches went through the slice-level
  `to_arrow`, which emits a tick type's full schema — an equity
  response gained always-null contract-identity columns, and every gRPC
  trade response gained four always-zero flag columns the wire never
  sent. Batches now project to the columns the response carried.
- **Interval values.** The picker offered `0`, `60s`, `300s` and
  `3600s`; ThetaData's v3 API accepts none of them. The list now comes
  from the backend, and stored specs written with the old spellings are
  translated on dispatch.
- **Flat-file datasets.** The Browse shelf offered eight archives when
  the service serves five, including four combinations that return
  `INVALID_PARAMS`, and omitted stock EOD, which is real. The shelf now
  reads the served matrix from the SDK, and the download command
  rejects an unserved pair with a message naming the alternatives.
- **Worker concurrency.** The pool spawned one sub-pool per asset class
  and summed their budgets, on the premise that ThetaData caps
  `2^tier` per class. It does not — the limiter is account-wide, so a
  Pro account ran up to 16 requests against a budget of 8, and the
  Settings screen advertised as many as 32 parallel downloads. There is
  now one pool sized by one account-wide budget.
- **Tier gating.** `tierForKind` consulted a five-entry client-side
  table and ignored the backend's verdicts entirely, so 55 of 60
  datasets reported as ungated regardless of subscription. Live
  verdicts now win, then the catalogue's own `min_tier`, with the
  static table as a pre-load fallback that reports `Unknown` rather
  than `Free`.
- **The scheduler fires.** `should_fire` had no caller, so schedules
  were stored and listed but never ran. It now runs on a one-minute
  ticker. It also ignored `cron_kind` — a `weekdays` schedule fired on
  Saturday and a `weekly:mon` schedule fired daily — and compared the
  fire time against UTC while documenting it as local, firing five
  hours early on the US east coast. Both are fixed and covered by
  tests.
- **Subscription tiers for indices and rates** report the account's
  real tier. They were hard-coded to `Unknown` against an older SDK
  that surfaced only two of the four.
- `sdk_version` and the Health panel no longer report a `tdbe` version
  for a crate that no longer exists.
- **A task whose file was already on disk was marked "empty"**, which is
  the status meaning the server returned no rows. The two outcomes are
  now distinct.
- **`dataset_metadata` could never succeed**: the UI sent `name` where
  the command took `operation_id`. It had no callers, so it is gone
  along with the lookup behind it.
- **The queue snapshot walked the entire output directory every 1.5
  seconds.** The footprint reading is now cached for 15 seconds; with a
  few thousand files that walk was the most expensive thing the app did
  at rest.
- **`cancel_many` mixed numbered and anonymous SQL placeholders**, so
  its bind positions depended on SQLite numbering rules rather than the
  text. Caught by the new bulk-operation tests.
- The index-preset modal had no Escape-to-close, unlike every other
  modal in the app.
- **Removing a queued task took two rounds** — cancel, reselect, remove
  — because removal refused live rows. It never needed to: every worker
  write is `WHERE id=? AND status='running' AND claimed_by=?`, so a
  deleted row makes them no-ops the worker already handles. Remove
  works in one step on anything.
- **The coverage heatmap invented the local side.** It received only the
  first and last date and filled in every weekday between them, drawing
  missing days as present — in the one view whose entire job is showing
  gaps. The scan's real date list is sent now. The same view was pinned
  to `symbol="QQQ"`, so it reported QQQ's coverage whatever dataset was
  open, and loaded twice on mount.
- **Links had no style at all**, falling back to the browser's default
  navy, which is unreadable on the dark theme.
- **The flat-file shelf and the index-preset shelf were never mounted.**
  Both features were complete, compiled and unreachable; they are on
  the Browse page now.
- Accessibility: clickable cards wrapped nested buttons in
  `role="button"`, the date-range handles carried slider ARIA on a
  button role, both sign-in forms were `<div>`s with a keydown handler
  instead of `<form>`s, and the command palette never pointed at its
  active option. `svelte-check` reports 0 errors and 0 warnings, from 1
  and 28.

### Changed

- **Visual identity is ThetaData's.** Palette, type scale, spacing,
  radii, action styling and motion now come from the ThetaData UI
  foundations sheet: brand blue `#1074FF` for identity, action blue
  `#0063CC` for links and filled buttons, Inter and IBM Plex Mono,
  4px spacing base, 6px input and 8px card radii, 150ms transitions.
  Light is the default theme; the dark variant keeps the same blue
  rather than a second identity. The previous periwinkle-indigo dark
  theme is gone.
- Colour literals are out of components. Every wash, ring and pill
  colour is a token, so both themes stay in step; 29 files carried
  hard-coded `rgba()` tuned for dark only.
- The subscription-tier pill palette is defined once instead of being
  copied into five components.
- **Tiers are coloured as a ladder.** They ran neutral, blue, green,
  amber: four unrelated hues that do not read as an order and collide
  with the status palette, since an amber Pro badge says *warning* and
  a green Standard badge says *success*. They now climb one blue by
  weight, with Pro the only filled chip. A cyan variant was tried and
  dropped — cyan sits next to blue on the wheel and muddies rather than
  ranks. Violet stays out: it is another vendor's identity.
- The dark theme's surfaces are deeper and cooler, and it no longer
  carries amber anywhere except genuine warnings.
- Queue tasks and the endpoint browser share one dispatcher. The
  worker's seven hand-written per-kind branches are gone.
- The wordmark is an inline component so one asset serves both themes.

### Added

- **API-key sign-in.** ThetaData accepts either credential; the app now
  does too, with a method switch on the login screen and the key stored
  in the same encrypted vault. `THETADATA_API_KEY` in the environment
  signs in without typing anything.
- **Library search, sort and gap counts.** The filter matches the
  dataset as well as the symbol, sorting covers symbol, size, file
  count and recency, there is an expand/collapse all, each row shows
  its format and how many weekdays inside its own span have no file,
  and one action fills every gap for a symbol at once.
- **A queue you can work with.** Checkbox selection with shift-click
  ranges, a select-all-visible header box, a bulk action bar (move to
  front, duplicate, retry, cancel, remove), a text filter over symbol,
  dataset and date, and "Clear finished". Bulk actions run in SQL
  against the whole queue rather than looping over the loaded page.
- **Row actions that act.** Cancel, move-to-front, duplicate and
  show-file were rendered as buttons with no handler attached; same for
  the Library's "re-run missing dates" and "open output directory".
  All six do their job now, and the library one queues exactly the
  trading days missing from a set's own span.
- `flatfile_datasets` and `interval_options` commands, so the UI reads
  both lists from the source of truth instead of keeping its own.
- Tests for the bulk queue operations, schedule cadence and fire-time,
  and interval normalisation against the SDK's own enum.

### Notes

- The catalogue currently resolves 60 endpoints against
  `thetadatadx-rs` 0.4. Earlier documentation said 61.
- The app links the SDK's `__internal` feature to reach the endpoint
  registry, which the SDK marks as unstable and not for external
  crates. The registry is what makes a data-driven catalogue possible,
  so the dependency is deliberate, but it pins this app to an SDK
  surface that can change without a major bump.

## Earlier

- Initial workspace: tdds-core engine, tdds-cli, apps/desktop (Tauri 2 +
  SvelteKit).
- Registry-driven dispatch over the ThetaData endpoint catalogue.
- Catalogue UI with subscription-tier gating + Upgrade CTA.
- Queue, scheduler, coverage map, parquet preview, health panel, DuckDB hint.
- Tauri plugins: single-instance, window-state, store, clipboard-manager,
  os, process, sql (sqlite), dialog, fs, opener, notification, stronghold,
  updater.
- CI workflows: Linux gate on push/PR + multi-platform release on tag.
