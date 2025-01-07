// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

pub mod analyze;
pub mod errors;
mod npm;
mod package_json;
mod path;
mod resolution;
mod sync;

pub use deno_package_json::PackageJson;
pub use npm::InNpmPackageChecker;
pub use npm::InNpmPackageCheckerRc;
pub use npm::NpmPackageFolderResolver;
pub use npm::NpmPackageFolderResolverRc;
pub use package_json::PackageJsonResolver;
pub use package_json::PackageJsonResolverRc;
pub use package_json::PackageJsonThreadLocalCache;
pub use path::PathClean;
pub use resolution::parse_npm_pkg_name;
pub use resolution::resolve_specifier_into_node_modules;
pub use resolution::IsBuiltInNodeModuleChecker;
pub use resolution::NodeResolution;
pub use resolution::NodeResolutionKind;
pub use resolution::NodeResolver;
pub use resolution::NodeResolverRc;
pub use resolution::ResolutionMode;
pub use resolution::DEFAULT_CONDITIONS;
pub use resolution::REQUIRE_CONDITIONS;
