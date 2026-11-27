//! Build-only Android packaging, source preparation and read-only device preflight.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod commands;
mod device;
mod package;
mod prepare;
mod process;

pub use commands::run;
