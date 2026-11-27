//! Browser shell. The host owns authentication and all database access.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(target_family = "wasm")]
mod binding;
#[cfg(any(target_family = "wasm", test))]
mod board_view;
#[cfg(any(target_family = "wasm", test))]
mod groups;
#[cfg(any(target_family = "wasm", test))]
mod session;
#[cfg(target_family = "wasm")]
mod shell;
#[cfg(target_family = "wasm")]
mod startup;
