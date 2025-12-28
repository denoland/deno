// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmRegistryApi;
use deno_npm_cache::TarballCache;
use deno_resolver::workspace::WorkspaceNpmLinkPackagesRc;
use deno_semver::package::PackageNv;

use crate::cache::DenoDir;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::sys::CliSys;

pub const ESBUILD_VERSION: &str = "0.25.5";

fn esbuild_platform() -> &'static str {
  match (std::env::consts::ARCH, std::env::consts::OS) {
    ("x86_64", "linux") => "linux-x64",
    ("aarch64", "linux") => "linux-arm64",
    ("x86_64", "macos" | "apple") => "darwin-x64",
    ("aarch64", "macos" | "apple") => "darwin-arm64",
    ("x86_64", "windows") => "win32-x64",
    ("aarch64", "windows") => "win32-arm64",
    ("x86_64", "android") => "android-x64",
    ("aarch64", "android") => "android-arm64",
    _ => panic!(
      "Unsupported platform: {} {}",
      std::env::consts::ARCH,
      std::env::consts::OS
    ),
  }
}

pub async fn ensure_esbuild(
  deno_dir: &DenoDir,
  npmrc: &ResolvedNpmRc,
  api: &Arc<CliNpmRegistryInfoProvider>,
  workspace_link_packages: &WorkspaceNpmLinkPackagesRc,
  tarball_cache: &Arc<TarballCache<CliNpmCacheHttpClient, CliSys>>,
  npm_cache: &CliNpmCache,
) -> Result<PathBuf, AnyError> {
  let target = esbuild_platform();
  let mut esbuild_path = deno_dir
    .dl_folder_path()
    .join(format!("esbuild-{}", ESBUILD_VERSION))
    .join(format!("esbuild-{}", target));
  if cfg!(windows) {
    esbuild_path.set_extension("exe");
  }

  if esbuild_path.exists() {
    return Ok(esbuild_path);
  }

  let pkg_name = format!("@esbuild/{}", target);
  let nv =
    PackageNv::from_str(&format!("{}@{}", pkg_name, ESBUILD_VERSION)).unwrap();
  let mut info = api.package_info(&pkg_name).await?;
  let version_info = match info.version_info(&nv, &workspace_link_packages.0) {
    Ok(version_info) => version_info,
    Err(_) => {
      api.mark_force_reload();
      info = api.package_info(&pkg_name).await?;
      info.version_info(&nv, &workspace_link_packages.0)?
    }
  };
  if let Some(dist) = &version_info.dist {
    let registry_url = npmrc.get_registry_url(&nv.name);
    let package_folder =
      npm_cache.package_folder_for_nv_and_url(&nv, registry_url);
    let existed = package_folder.exists();

    if !existed {
      tarball_cache
        .ensure_package(&nv, dist)
        .await
        .with_context(|| {
          format!(
            "failed to download esbuild package tarball {} from {}",
            nv, dist.tarball
          )
        })?;
    }

    let path = if cfg!(windows) {
      package_folder.join("esbuild.exe")
    } else {
      package_folder.join("bin").join("esbuild")
    };

    std::fs::create_dir_all(esbuild_path.parent().unwrap()).with_context(
      || {
        format!(
          "failed to create directory {}",
          esbuild_path.parent().unwrap().display()
        )
      },
    )?;
    std::fs::copy(&path, &esbuild_path).with_context(|| {
      format!(
        "failed to copy esbuild binary from {} to {}",
        path.display(),
        esbuild_path.display()
      )
    })?;

    if !existed {
      let _ = std::fs::remove_dir_all(&package_folder).inspect_err(|e| {
        log::warn!(
          "failed to remove directory {}: {}",
          package_folder.display(),
          e
        );
      });
    }
    Ok(esbuild_path)
  } else {
    anyhow::bail!(
      "could not get fetch esbuild binary; download it manually and copy it to {}",
      esbuild_path.display()
    );
  }
}
