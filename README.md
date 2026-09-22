# TdDx Store

A queue-driven dataset downloader for [ThetaData](https://thetadata.net).
Cross-platform desktop app + companion CLI, built on Tauri 2 + SvelteKit
over the [ThetaDataDx SDK](https://crates.io/crates/thetadatadx-rs).

Browse the dataset catalogue — stocks, options, indices and rates, across
trades, NBBO quotes, OHLC, EOD, greeks and open interest — queue
downloads, watch live progress, and write parquet / csv / json / jsonl.
The catalogue is the SDK's own endpoint registry, so it never advertises
a dataset the server does not serve. The app reads your subscription
tiers at connect time, greys out what your plan does not cover, links to
the upgrade page, and ships scheduling, coverage maps, parquet preview
and a health panel.

## Quick start

```bash
# Desktop app (dev)
cd apps/desktop
npm install
npm run tauri dev

# CLI
cargo run -p tdds-cli -- --help
```

The CLI binary is `tddx-store`.

## Workspace layout

```
.
├── apps/desktop/          # Tauri 2 + SvelteKit + Vite + TS
│   ├── src/               # Svelte UI (feature-grouped under src/lib/)
│   └── src-tauri/         # Rust backend (commands/ split by domain)
├── crates/
│   ├── tdds-core/         # Engine: queue, worker pool, registry, format,
│   │                      # coverage, schedule, tier-gating, …
│   └── tdds-cli/          # `tddx-store` CLI on top of tdds-core
├── .github/workflows/     # ci.yml (push/PR) + release.yml (tag-driven matrix)
└── DESIGN.md ROADMAP.md
```

## How it talks to ThetaData

One dependency: `thetadatadx-rs` from crates.io, with the `arrow`
feature. Its lib target is `thetadatadx`, which is the name every `use`
site spells.

- **Market data** goes through the SDK's endpoint registry. One
  dispatcher (`tdds_core::registry`) serves the queue, the endpoint
  runner, list queries and schedules, so argument validation and Arrow
  column projection happen in exactly one place.
- **Flat files** are the daily bulk archives. ThetaData serves five
  datasets and rejects everything else: option `trade_quote` /
  `open_interest` / `eod` and stock `trade_quote` / `eod`. The Browse
  shelf reads that list from the SDK rather than hard-coding it, and the
  download command re-checks before spending a round-trip.
- **Concurrency** is one account-wide budget of `2^tier` in-flight
  requests, taken from your highest per-class tier. It is not the sum of
  the per-class budgets: ThetaData's limiter is account-wide, and
  requests past the budget are queued and paced server-side rather than
  processed in parallel.

## Design

The palette, type scale, spacing, radii and action styling come from the
ThetaData UI foundations sheet, compressed to working density. Light is
the default theme; a dark variant ships for long sessions. Details and
the full token set are in [DESIGN.md](DESIGN.md).

## Releases

Tag a `v*.*.*` to fan out a multi-platform build via
`tauri-apps/tauri-action`:

```bash
git tag v0.0.1 -m "first release"
git push --tags
```

The release workflow produces a draft GitHub release with:
- macOS `.dmg` + `.app` (universal, Apple Silicon + Intel)
- Linux `.deb` + `.AppImage` + `.rpm`
- Windows `.msi` + `.exe` (NSIS)

Unsigned binaries trigger Gatekeeper / SmartScreen warnings on first
launch. Apple Developer ID + Authenticode signing wire in via
`tauri-action` secrets when available.

## Tauri plugins in use

`single-instance`, `window-state`, `store`, `clipboard-manager`, `os`,
`process`, `sql` (sqlite), `dialog`, `fs`, `opener`, `notification`,
`stronghold`, `updater`. All paths resolve via
`app.path().app_local_data_dir()` (XDG on Linux, Application Support on
macOS, LOCALAPPDATA on Windows).

## License

MIT. See [LICENSE](LICENSE).
