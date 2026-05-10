# Changelog

All notable changes to TdDx Store are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial workspace: tdds-core engine, tdds-cli, apps/desktop (Tauri 2 + SvelteKit).
- 61 ThetaData endpoints surfaced via the registry-driven dispatcher.
- Curated catalogue UI with subscription-tier gating + Upgrade CTA.
- Queue, scheduler, coverage map, parquet preview, health panel, DuckDB hint.
- Tauri plugins: single-instance, window-state, store, clipboard-manager,
  os, process, sql (sqlite), dialog, fs, opener, notification, stronghold,
  updater.
- CI workflows: Linux gate on push/PR + multi-platform release on tag.
