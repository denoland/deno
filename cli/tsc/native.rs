// Copyright 2018-2026 the Deno authors. MIT license.

// The native `deno check` path that consumes this module lands on top of this
// PR, so everything here is unused until then.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_npm::registry::NpmRegistryApi;
use deno_npm_cache::TarballCache;
use deno_npmrc::ResolvedNpmRc;
use deno_resolver::workspace::WorkspaceNpmLinkPackagesRc;
use deno_semver::package::PackageNv;

use crate::cache::DenoDir;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::sys::CliSys;

/// Pinned version of the native TypeScript compiler (`typescript@N`, the
/// Go/`tsgo` line) that `deno check` runs. Bumping this constant is the only
/// supported way to change the compiler; `deno check` never floats to the
/// latest published version so that a given Deno release always type-checks
/// with a known compiler.
pub const TYPESCRIPT_VERSION: &str = "7.0.2";

/// npm platform-package suffix for the current host, matching the
/// `@typescript/typescript-<suffix>` optional dependencies shipped by
/// `typescript`.
fn typescript_platform() -> Result<&'static str, AnyError> {
  typescript_platform_for(std::env::consts::ARCH, std::env::consts::OS)
}

/// Map a Rust `(ARCH, OS)` pair (as in [`std::env::consts`]) to the
/// `@typescript/typescript-<suffix>` package suffix. Split out so the mapping
/// can be unit-tested for every supported host.
fn typescript_platform_for(
  arch: &str,
  os: &str,
) -> Result<&'static str, AnyError> {
  Ok(match (arch, os) {
    ("x86_64", "linux") => "linux-x64",
    ("aarch64", "linux") => "linux-arm64",
    ("x86_64", "macos") => "darwin-x64",
    ("aarch64", "macos") => "darwin-arm64",
    ("x86_64", "windows") => "win32-x64",
    ("aarch64", "windows") => "win32-arm64",
    _ => anyhow::bail!(
      "Unsupported platform for the native TypeScript compiler: {arch} {os}"
    ),
  })
}

/// Ensure the pinned native `tsc` for the host platform is available and
/// return the path to the executable, downloading the
/// `@typescript/typescript-<platform>` npm package if it isn't cached yet.
///
/// This deliberately fetches only the single host-platform package rather than
/// resolving `typescript` itself (whose optional dependencies would pull every
/// platform binary), mirroring how [`crate::tools::bundle::esbuild`] obtains
/// esbuild. Unlike esbuild (a single standalone binary), the tsc binary lives
/// in a `lib/` directory next to the default `lib.*.d.ts` files it loads at
/// runtime, so the whole `lib/` tree is materialized alongside it.
pub async fn ensure_native_tsc(
  deno_dir: &DenoDir,
  npmrc: &ResolvedNpmRc,
  api: &Arc<CliNpmRegistryInfoProvider>,
  workspace_link_packages: &WorkspaceNpmLinkPackagesRc,
  tarball_cache: &Arc<TarballCache<CliNpmCacheHttpClient, CliSys>>,
  npm_cache: &CliNpmCache,
) -> Result<PathBuf, AnyError> {
  // Allow pointing at an already-available `tsc` binary instead of downloading
  // one, mirroring `DENORT_BIN`. Used by the test harness and CI to avoid
  // re-downloading the compiler for every run, and lets a user supply their
  // own build.
  if let Some(path) = std::env::var_os("DENO_TSC_BIN") {
    let path = PathBuf::from(path);
    if path.exists() {
      return Ok(path);
    }
    log::warn!(
      "DENO_TSC_BIN is set to {} but it does not exist; downloading the pinned compiler instead",
      path.display()
    );
  }

  let target = typescript_platform()?;
  // Keep the compiler under `$DENO_DIR/tsc/<version>/<platform>` so all of a
  // given version's files live in one predictable, versioned directory.
  let install_dir = deno_dir
    .root
    .join("tsc")
    .join(TYPESCRIPT_VERSION)
    .join(target);
  let bin_name = if cfg!(windows) { "tsc.exe" } else { "tsc" };
  let tsc_path = install_dir.join("lib").join(bin_name);

  if tsc_path.exists() {
    return Ok(tsc_path);
  }

  let pkg_name = format!("@typescript/typescript-{}", target);
  let nv = PackageNv::from_str(&format!("{}@{}", pkg_name, TYPESCRIPT_VERSION))
    .unwrap();
  let mut info = api.package_info(&pkg_name).await?;
  let version_info = match info.version_info(&nv, &workspace_link_packages.0) {
    Ok(version_info) => version_info,
    Err(_) => {
      api.mark_force_reload();
      info = api.package_info(&pkg_name).await?;
      info.version_info(&nv, &workspace_link_packages.0)?
    }
  };
  let Some(dist) = &version_info.dist else {
    anyhow::bail!(
      "could not resolve the native TypeScript compiler; download {} manually and copy its lib/ next to {}",
      nv,
      tsc_path.display()
    );
  };

  let registry_url = npmrc.get_registry_url(&nv.name);
  let package_folder =
    npm_cache.package_folder_for_nv_and_url(&nv, registry_url);
  let existed = package_folder.exists();
  if !existed {
    // `ensure_package` downloads the tarball and verifies it against the
    // registry `dist` integrity/shasum before extracting (the standard npm
    // pipeline), so the materialized compiler is checksum-validated at install.
    tarball_cache
      .ensure_package(&nv, dist)
      .await
      .with_context(|| {
        format!(
          "failed to download the TypeScript compiler tarball {} from {}",
          nv, dist.tarball
        )
      })?;
  }

  // Materialize `lib/` (the native binary plus the default lib `.d.ts` files it
  // resolves relative to itself) atomically: copy into a sibling temp dir and
  // rename it into place. The rename is atomic, so a concurrent `deno check`
  // never observes a half-copied tree through the `exists()` check above.
  let version_dir = install_dir.parent().unwrap();
  std::fs::create_dir_all(version_dir).with_context(|| {
    format!("failed to create directory {}", version_dir.display())
  })?;
  let tmp_dir =
    version_dir.join(format!(".{}-{}.tmp", target, std::process::id()));
  let _ = std::fs::remove_dir_all(&tmp_dir);
  crate::tools::compile::copy_dir_all(
    &package_folder.join("lib"),
    &tmp_dir.join("lib"),
  )
  .with_context(|| {
    format!(
      "failed to copy the TypeScript compiler out of {}",
      package_folder.display()
    )
  })?;
  match std::fs::rename(&tmp_dir, &install_dir) {
    Ok(()) => {}
    // Another process won the race and installed it already; discard our copy.
    Err(_) if tsc_path.exists() => {
      let _ = std::fs::remove_dir_all(&tmp_dir);
    }
    Err(err) => {
      let _ = std::fs::remove_dir_all(&tmp_dir);
      return Err(err).with_context(|| {
        format!(
          "failed to move the TypeScript compiler into place at {}",
          install_dir.display()
        )
      });
    }
  }

  if !existed {
    let _ = std::fs::remove_dir_all(&package_folder).inspect_err(|e| {
      log::warn!(
        "failed to remove directory {}: {}",
        package_folder.display(),
        e
      )
    });
  }

  Ok(tsc_path)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn typescript_platform_mapping() {
    assert_eq!(
      typescript_platform_for("x86_64", "linux").unwrap(),
      "linux-x64"
    );
    assert_eq!(
      typescript_platform_for("aarch64", "linux").unwrap(),
      "linux-arm64"
    );
    assert_eq!(
      typescript_platform_for("x86_64", "macos").unwrap(),
      "darwin-x64"
    );
    assert_eq!(
      typescript_platform_for("aarch64", "macos").unwrap(),
      "darwin-arm64"
    );
    assert_eq!(
      typescript_platform_for("x86_64", "windows").unwrap(),
      "win32-x64"
    );
    assert_eq!(
      typescript_platform_for("aarch64", "windows").unwrap(),
      "win32-arm64"
    );
  }

  #[test]
  fn typescript_platform_unsupported() {
    let err = typescript_platform_for("riscv64", "linux").unwrap_err();
    assert!(err.to_string().contains("Unsupported platform"));
    // The host's own platform must be supported (this is what CI runs on).
    assert!(typescript_platform().is_ok());
  }
}
