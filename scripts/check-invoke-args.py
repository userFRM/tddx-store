#!/usr/bin/env python3
"""Fail when a frontend `invoke()` passes argument keys Tauri will reject.

Tauri 2 exposes a Rust command parameter `output_dir` to JavaScript as
`outputDir`. Passing the snake_case key compiles, type-checks, and then
fails at runtime with "missing required key outputDir" — which is how
the Library's DuckDB export shipped broken. Nothing in `svelte-check`
or `cargo check` can see across that boundary, so this does.

Checks every command whose parameters are plain values (struct-typed
`args` parameters deserialise by serde field name and are exempt).
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RUST = ROOT / "apps/desktop/src-tauri/src/commands"
TS = ROOT / "apps/desktop/src/lib/api/index.ts"
IGNORED = {"state", "app", "_app", "window"}


def camel(name: str) -> str:
    head, *rest = name.split("_")
    return head + "".join(w.capitalize() for w in rest)


rust = "".join(p.read_text() for p in sorted(RUST.glob("*.rs")))
ts = TS.read_text()
commands = re.findall(
    r"#\[tauri::command[^\]]*\]\s*pub (?:async )?fn (\w+)\(([^)]*)\)", rust, re.S
)

problems = []
for name, params in commands:
    names = [p.split(":")[0].strip() for p in params.split(",") if ":" in p]
    names = [n for n in names if n not in IGNORED]
    call = re.search(r'invoke[^(]*\(\s*"%s"\s*(?:,\s*\{([^}]*)\})?' % name, ts)
    if not call or call.group(1) is None:
        continue
    passed = {k.strip().split(":")[0].strip() for k in call.group(1).split(",") if k.strip()}
    for n in names:
        expected = camel(n)
        if n != expected and n in passed:
            problems.append(f"{name}: passes `{n}`, Tauri expects `{expected}`")

if problems:
    print("invoke argument keys that Tauri will reject at runtime:")
    for p in problems:
        print("  " + p)
    sys.exit(1)
print(f"checked {len(commands)} commands: argument keys match")
