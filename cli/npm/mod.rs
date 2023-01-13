// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod cache;
mod registry;
mod resolution;
mod resolvers;
mod semver;
mod tarball;

#[cfg(test)]
pub use self::semver::NpmVersion;
pub use cache::NpmCache;
#[cfg(test)]
pub use registry::NpmPackageVersionDistInfo;
pub use registry::NpmRegistryApi;
pub use registry::RealNpmRegistryApi;
pub use resolution::resolve_npm_package_reqs;
pub use resolution::NpmPackageId;
pub use resolution::NpmPackageReference;
pub use resolution::NpmPackageReq;
pub use resolution::NpmResolutionPackage;
pub use resolution::NpmResolutionSnapshot;
pub use resolvers::NpmPackageResolver;
