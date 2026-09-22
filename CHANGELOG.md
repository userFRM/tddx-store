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
- Queue tasks and the endpoint browser share one dispatcher. The
  worker's seven hand-written per-kind branches are gone.
- The wordmark is an inline component so one asset serves both themes.

### Added

- `flatfile_datasets` and `interval_options` commands, so the UI reads
  both lists from the source of truth instead of keeping its own.
- Tests for schedule cadence and fire-time, and for interval
  normalisation against the SDK's own enum.

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
