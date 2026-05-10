//! Tauri command handlers, grouped by domain.
//!
//! Each module exposes a small, related set of `#[tauri::command]`
//! functions. `lib.rs` wires them into the invoke handler — never
//! re-exports the inner types so each command's signature is local
//! to its module.

pub mod connection;
pub mod coverage;
pub mod endpoints;
pub mod flatfiles;
pub mod health;
pub mod index_presets;
pub mod preview;
pub mod queue;
pub mod schedule;
pub mod settings;
pub mod tier;
pub mod vault;
