//! Browser shell. The host owns authentication and all database access.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(target_family = "wasm")]
mod binding;
#[cfg(target_family = "wasm")]
mod board_view;
#[cfg(target_family = "wasm")]
mod groups;
#[cfg(target_family = "wasm")]
mod session;
#[cfg(target_family = "wasm")]
mod shell;
#[cfg(target_family = "wasm")]
mod startup;
