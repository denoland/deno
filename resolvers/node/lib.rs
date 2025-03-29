// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

pub mod analyze;
mod builtin_modules;
pub mod cache;
pub mod errors;
mod npm;
mod package_json;
mod path;
mod resolution;

mod sync;

pub use builtin_modules::DenoIsBuiltInNodeModuleChecker;
pub use builtin_modules::IsBuiltInNodeModuleChecker;
pub use builtin_modules::DENO_SUPPORTED_BUILTIN_NODE_MODULES;
pub use cache::NodeResolutionCache;
pub use cache::NodeResolutionCacheRc;
pub use deno_package_json::PackageJson;
pub use npm::InNpmPackageChecker;
pub use npm::NpmPackageFolderResolver;
pub use package_json::PackageJsonCacheRc;
pub use package_json::PackageJsonResolver;
pub use package_json::PackageJsonResolverRc;
pub use package_json::PackageJsonThreadLocalCache;
pub use path::PathClean;
pub use path::UrlOrPath;
pub use path::UrlOrPathRef;
pub use resolution::parse_npm_pkg_name;
pub use resolution::resolve_specifier_into_node_modules;
pub use resolution::types_package_name;
pub use resolution::ConditionsFromResolutionMode;
pub use resolution::NodeResolution;
pub use resolution::NodeResolutionKind;
pub use resolution::NodeResolver;
pub use resolution::NodeResolverOptions;
pub use resolution::NodeResolverRc;
pub use resolution::ResolutionMode;
pub use resolution::DEFAULT_CONDITIONS;
pub use resolution::REQUIRE_CONDITIONS;
