# Contributing

Thanks for your interest in TdDx Store. This is a small project — the
contribution loop is short.

## Local setup

```bash
git clone https://github.com/userFRM/tddx-store.git
cd tddx-store

# Rust
cargo check --workspace

# Frontend (desktop app)
cd apps/desktop
npm install
npm run check        # svelte-check
npm run tauri dev    # full app, hot-reload
```

System packages on Linux: `libgtk-3-dev libwebkit2gtk-4.1-dev
libsoup-3.0-dev libjavascriptcoregtk-4.1-dev
libayatana-appindicator3-dev librsvg2-dev patchelf protobuf-compiler`.

## Quality bar

Before pushing:

```bash
# From the workspace root
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# In apps/desktop
npm run check
npm run build
```

CI runs the same gates on Linux on every push / PR. The release
workflow runs the full macOS / Linux / Windows matrix on tag.

## Branching

Trunk-based. Commit straight to `main` for small fixes; open a PR for
anything non-trivial. Conventional Commits (`feat:`, `fix:`,
`chore:`, `docs:`, `refactor:`) for the title.

## Releases

Push a SemVer-prefixed tag:

```bash
git tag v0.0.X -m "release notes"
git push --tags
```

`tauri-apps/tauri-action` builds + drafts the release. Review the
notes in the GitHub UI, then publish.
