# TdDx Store

A clean, queue-driven dataset downloader for [ThetaData](https://thetadata.net).
Cross-platform desktop app + companion CLI built on Tauri 2 + SvelteKit.

Browse a curated catalogue of stock + option datasets (trades, NBBO quotes,
OHLC, EOD, greeks, open interest — all 61 ThetaData endpoints), queue
downloads, watch live progress, and persist to parquet / csv / jsonl. The
app respects your subscription tier, surfaces an Upgrade link when a
dataset is gated, and ships with scheduling, coverage maps, parquet
preview, and a health panel.

## Quick start

```bash
# Desktop app (dev)
cd apps/desktop
npm install
npm run tauri dev

# CLI
cargo run -p tdds-cli -- --help
```

## Workspace layout

```
.
├── apps/desktop/          # Tauri 2 + SvelteKit + Vite + TS
│   ├── src/               # Svelte UI (feature-grouped under src/lib/)
│   └── src-tauri/         # Rust backend (commands/ split by domain)
├── crates/
│   ├── tdds-core/         # Engine: queue, worker pool, registry, format,
│   │                      # coverage, schedule, tier-gating, secrets, …
│   └── tdds-cli/          # `tddl`/`tddl-cli` CLI on top of tdds-core
├── .github/workflows/     # ci.yml (push/PR) + release.yml (tag-driven matrix)
└── DESIGN.md ROADMAP.md
```

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
launch. Apple Developer ID + Authenticode signing wired in via
`tauri-action` secrets when available.

## Tauri plugins in use

`single-instance`, `window-state`, `store`, `clipboard-manager`, `os`,
`process`, `sql` (sqlite), `dialog`, `fs`, `opener`, `notification`,
`stronghold`, `updater`. All paths resolved via
`app.path().app_data_dir()` (XDG on Linux, Application Support on macOS,
APPDATA on Windows).

## License

MIT. See [LICENSE](LICENSE).
