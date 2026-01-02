// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]
#![deny(clippy::unused_async)]
#![deny(clippy::unnecessary_wraps)]

#[cfg(feature = "deno_json")]
pub mod deno_json;
#[cfg(feature = "deno_json")]
pub mod glob;
#[cfg(feature = "deno_json")]
pub mod import_map;
#[cfg(feature = "deno_json")]
mod util;
#[cfg(feature = "workspace")]
pub mod workspace;

#[cfg(feature = "deno_json")]
pub use deno_path_util::UrlToFilePathError;
#[cfg(feature = "deno_json")]
pub use util::ParseDateOrDurationError;
#[cfg(feature = "deno_json")]
pub use util::ParseIso8601DurationError;
#[cfg(feature = "deno_json")]
pub use util::parse_minutes_duration_or_date;
