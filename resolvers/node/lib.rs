// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

pub mod analyze;
pub mod env;
pub mod errors;
mod npm;
mod package_json;
mod path;
mod resolution;
mod sync;

pub use deno_package_json::PackageJson;
pub use npm::NpmResolver;
pub use npm::NpmResolverRc;
pub use package_json::load_pkg_json;
pub use package_json::PackageJsonThreadLocalCache;
pub use path::PathClean;
pub use resolution::parse_npm_pkg_name;
pub use resolution::NodeModuleKind;
pub use resolution::NodeResolution;
pub use resolution::NodeResolutionMode;
pub use resolution::NodeResolver;
pub use resolution::DEFAULT_CONDITIONS;
pub use resolution::REQUIRE_CONDITIONS;
