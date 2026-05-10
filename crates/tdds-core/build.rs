//! Codegen full per-endpoint metadata from the authoritative ThetaData
//! OpenAPI spec. Vendored `spec/openapiv3.yaml` is the build-time
//! source; the same parser runs at app launch over the freshly-fetched
//! upstream copy via `tier::fetch_and_install_remote`.
//!
//! Output: `$OUT_DIR/yaml_metadata_generated.rs`, included by
//! `src/yaml_meta.rs`. Emits:
//!   - `min_tier_for_endpoint_generated(op) -> Option<Tier>` — tier table
//!   - `endpoint_meta_generated(op) -> Option<&'static EndpointMetaStatic>`
//!     — per-endpoint summary + description + tag for UI.
//!
//! Plain-text scan rather than a YAML parser crate: the spec contains
//! multi-line single-quoted scalars in response examples that strict
//! YAML 1.2 parsers (saphyr, yaml-rust2) reject. Plain-text scan over
//! `paths:` is robust + dependency-free.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Default, Debug)]
struct EndpointMeta {
    tier: Option<&'static str>,
    summary: Option<String>,
    description: Option<String>,
    tag: Option<String>,
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let spec_path = manifest_dir.join("spec").join("openapiv3.yaml");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set"));

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", spec_path.display());

    let yaml = fs::read_to_string(&spec_path)
        .unwrap_or_else(|err| panic!("read spec at {}: {err}", spec_path.display()));

    let table = parse_yaml(&yaml).unwrap_or_else(|err| panic!("parse spec: {err}"));
    if table.is_empty() {
        panic!("no endpoints with x-min-subscription found in spec");
    }

    let generated_path = out_dir.join("yaml_metadata_generated.rs");
    let mut body = String::with_capacity(table.len() * 256);
    body.push_str("/// Auto-generated from `spec/openapiv3.yaml` by `build.rs`. Do not edit.\n");
    body.push_str("#[doc(hidden)]\n");
    body.push_str("pub fn min_tier_for_endpoint_generated(operation_id: &str) -> Option<Tier> {\n");
    body.push_str("    let t = match operation_id {\n");
    for (op, meta) in &table {
        if let Some(tier) = meta.tier {
            body.push_str("        \"");
            body.push_str(op);
            body.push_str("\" => Tier::");
            body.push_str(tier);
            body.push_str(",\n");
        }
    }
    body.push_str("        _ => return None,\n");
    body.push_str("    };\n");
    body.push_str("    Some(t)\n");
    body.push_str("}\n\n");

    body.push_str("/// Static slice of (operationId, summary, description, tag, tier).\n");
    body.push_str("#[doc(hidden)]\n");
    body.push_str(
        "pub const ENDPOINT_META_TABLE: &[(&str, &str, &str, &str, Option<Tier>)] = &[\n",
    );
    for (op, meta) in &table {
        body.push_str("    (\"");
        body.push_str(op);
        body.push_str("\", ");
        body.push_str(&rust_string_lit(meta.summary.as_deref().unwrap_or("")));
        body.push_str(", ");
        body.push_str(&rust_string_lit(meta.description.as_deref().unwrap_or("")));
        body.push_str(", ");
        body.push_str(&rust_string_lit(meta.tag.as_deref().unwrap_or("")));
        body.push_str(", ");
        match meta.tier {
            Some(t) => {
                body.push_str("Some(Tier::");
                body.push_str(t);
                body.push(')');
            }
            None => body.push_str("None"),
        }
        body.push_str("),\n");
    }
    body.push_str("];\n\n");

    body.push_str("/// Number of endpoints in the generated catalogue.\n");
    body.push_str("#[doc(hidden)]\n");
    body.push_str(&format!(
        "pub const ENDPOINT_META_COUNT: usize = {};\n",
        table.len()
    ));

    fs::write(&generated_path, body)
        .unwrap_or_else(|err| panic!("write {}: {err}", generated_path.display()));
}

/// Walk the OpenAPI document. State machine over indent levels:
///   `paths:` (top-level)
///     `  /<path>:` (2-space)
///       `    x-min-subscription: <tier>` (4-space)
///       `    get:` (4-space)
///         `      summary: <text>` (6-space)
///         `      description: <text>` OR `      description: |` + indented block
///         `      tags:` (6-space) followed by `        - <tag>`
fn parse_yaml(yaml: &str) -> Result<BTreeMap<String, EndpointMeta>, String> {
    let mut out: BTreeMap<String, EndpointMeta> = BTreeMap::new();
    let mut in_paths = false;
    let mut current: Option<String> = None;
    let mut in_get_block = false;
    let mut collecting_description: Option<usize> = None; // base indent
    let mut description_buf: Vec<String> = Vec::new();
    let mut in_tags_block = false;
    let mut waiting_first_tag = false;

    for raw_line in yaml.lines() {
        // Top-level key flips section.
        if !raw_line.starts_with(' ') && raw_line.trim_end().ends_with(':') {
            in_paths = raw_line.trim_start().starts_with("paths:");
            current = None;
            in_get_block = false;
            collecting_description = None;
            in_tags_block = false;
            continue;
        }
        if !in_paths {
            continue;
        }

        let trimmed = raw_line.trim_start();
        let indent = raw_line.len() - trimmed.len();

        // Multi-line description block: collect lines deeper than the
        // `description: |` opener.
        if let Some(base_indent) = collecting_description {
            if trimmed.is_empty() {
                description_buf.push(String::new());
                continue;
            }
            if indent > base_indent {
                description_buf.push(trimmed.trim_end().to_string());
                continue;
            }
            // End of block — flush.
            if let Some(op) = current.as_ref() {
                let entry = out.entry(op.clone()).or_default();
                if entry.description.is_none() {
                    entry.description = Some(description_buf.join(" ").trim().to_string());
                }
            }
            description_buf.clear();
            collecting_description = None;
        }

        // Path line: 2-space indent, leading slash, trailing colon.
        if indent == 2 && trimmed.starts_with('/') && raw_line.trim_end().ends_with(':') {
            let raw_path = trimmed.trim_end().trim_end_matches(':');
            let key = path_to_op_id(raw_path);
            current = if key.is_empty() { None } else { Some(key) };
            in_get_block = false;
            in_tags_block = false;
            continue;
        }

        let Some(op) = current.as_ref().cloned() else {
            continue;
        };

        // Per-path tier (4-space indent under the path).
        if indent == 4 {
            if let Some(rest) = trimmed.strip_prefix("x-min-subscription:") {
                let raw_tier = rest.trim();
                let tier = match raw_tier.to_ascii_lowercase().as_str() {
                    "free" => Some("Free"),
                    "value" => Some("Value"),
                    "standard" => Some("Standard"),
                    "pro" | "professional" => Some("Pro"),
                    other if !other.is_empty() => {
                        return Err(format!("unknown tier `{other}` on {op}"));
                    }
                    _ => None,
                };
                if let Some(t) = tier {
                    out.entry(op.clone()).or_default().tier = Some(t);
                }
                continue;
            }
            if trimmed.starts_with("get:") {
                in_get_block = true;
                in_tags_block = false;
                continue;
            }
            // Any other 4-space top-level under path closes get block.
            in_get_block = false;
            in_tags_block = false;
            continue;
        }

        // 6-space fields under `get:`.
        if in_get_block && indent == 6 {
            in_tags_block = false;
            if let Some(rest) = trimmed.strip_prefix("summary:") {
                let val = rest.trim();
                if !val.is_empty() {
                    let entry = out.entry(op.clone()).or_default();
                    if entry.summary.is_none() {
                        entry.summary = Some(unquote(val).to_string());
                    }
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("description:") {
                let val = rest.trim();
                if val == "|" || val == "|-" || val == ">" || val == ">-" {
                    collecting_description = Some(indent);
                    description_buf.clear();
                } else if !val.is_empty() {
                    let entry = out.entry(op.clone()).or_default();
                    if entry.description.is_none() {
                        entry.description = Some(unquote(val).to_string());
                    }
                }
                continue;
            }
            if trimmed.starts_with("tags:") {
                in_tags_block = true;
                waiting_first_tag = true;
                continue;
            }
            continue;
        }

        // Tag list: `        - <tag>` at 8-space indent. Take first only.
        if in_tags_block && indent == 8 && trimmed.starts_with("- ") && waiting_first_tag {
            let tag = trimmed[2..].trim();
            let entry = out.entry(op.clone()).or_default();
            if entry.tag.is_none() {
                entry.tag = Some(unquote(tag).to_string());
            }
            waiting_first_tag = false;
            in_tags_block = false;
        }
    }

    // Drop entries that never got a tier — they're either non-endpoint
    // metadata (components, info, …) or deprecated paths.
    out.retain(|_, m| m.tier.is_some());
    Ok(out)
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn rust_string_lit(s: &str) -> String {
    let escaped: String = s
        .chars()
        .flat_map(|c| match c {
            '\\' => vec!['\\', '\\'],
            '"' => vec!['\\', '"'],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            '\t' => vec!['\\', 't'],
            c if (c as u32) < 0x20 => vec![],
            c => vec![c],
        })
        .collect();
    format!("\"{escaped}\"")
}

fn path_to_op_id(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg.starts_with('{') {
            continue;
        }
        parts.push(seg);
    }
    parts.join("_")
}
