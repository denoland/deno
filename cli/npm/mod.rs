// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod cache;
mod registry;
mod resolution;
mod resolvers;
mod tarball;

pub use cache::NpmCache;
#[cfg(test)]
pub use registry::NpmPackageVersionDistInfo;
pub use registry::NpmRegistryApi;
#[cfg(test)]
pub use registry::TestNpmRegistryApi;
pub use resolution::resolve_graph_npm_info;
pub use resolution::NpmResolution;
pub use resolution::NpmResolutionPackage;
pub use resolution::NpmResolutionSnapshot;
pub use resolvers::NpmPackageResolver;
