// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::NpmVersionResolver;
use deno_semver::npm::NpmPackageReqReference;

use crate::http_util::HttpClientProvider;

pub struct BinNameResolver<'a> {
  http_client_provider: &'a HttpClientProvider,
  npm_registry_api: &'a dyn NpmRegistryApi,
  npm_version_resolver: &'a NpmVersionResolver,
}

impl<'a> BinNameResolver<'a> {
  pub fn new(
    http_client_provider: &'a HttpClientProvider,
    npm_registry_api: &'a dyn NpmRegistryApi,
    npm_version_resolver: &'a NpmVersionResolver,
  ) -> Self {
    Self {
      http_client_provider,
      npm_registry_api,
      npm_version_resolver,
    }
  }

  pub async fn infer_name_from_url(&self, url: &Url) -> Option<String> {
    // If there's an absolute url with no path, eg. https://my-cli.com
    // perform a request, and see if it redirects another file instead.
    let mut url = url.clone();

    if matches!(url.scheme(), "http" | "https")
      && url.path() == "/"
      && let Ok(client) = self.http_client_provider.get_or_create()
      && let Ok(redirected_url) = client
        .get_redirected_url(url.clone(), &Default::default())
        .await
    {
      url = redirected_url;
    }

    if let Ok(npm_ref) = NpmPackageReqReference::from_specifier(&url) {
      if let Some(sub_path) = npm_ref.sub_path()
        && !sub_path.contains('/')
      {
        return Some(sub_path.to_string());
      }

      match self.resolve_name_from_npm(&npm_ref).await {
        Ok(Some(value)) => return Some(value),
        Ok(None) => {}
        Err(err) => {
          log::warn!(
            "{} Failed resolving npm specifier information. {:#}",
            deno_runtime::colors::yellow("Warning"),
            err
          );
        }
      }

      if !npm_ref.req().name.contains('/') {
        return Some(npm_ref.into_inner().req.name.into_string());
      }
      if let Some(scope_and_pkg) = npm_ref.req().name.strip_prefix('@')
        && let Some((scope, package)) = scope_and_pkg.split_once('/')
        && package == "cli"
      {
        return Some(scope.to_string());
      }

      return None;
    }

    let percent_decode =
      percent_encoding::percent_decode(url.path().as_bytes());
    #[cfg(unix)]
    let path = {
      use std::os::unix::prelude::OsStringExt;
      PathBuf::from(std::ffi::OsString::from_vec(
        percent_decode.collect::<Vec<u8>>(),
      ))
    };
    #[cfg(windows)]
    let path = PathBuf::from(percent_decode.decode_utf8_lossy().as_ref());

    let mut stem = path.file_stem()?.to_string_lossy();
    if matches!(stem.as_ref(), "main" | "mod" | "index" | "cli")
      && let Some(parent_name) = path.parent().and_then(|p| p.file_name())
    {
      stem = parent_name.to_string_lossy();
    }

    // if atmark symbol appears in the index other than 0 (e.g. `foo@bar`) we use
    // the former part as the inferred name because the latter part is most likely
    // a version number.
    match stem.find('@') {
      Some(at_index) if at_index > 0 => {
        stem = stem.split_at(at_index).0.to_string().into();
      }
      _ => {}
    }

    Some(stem.to_string())
  }

  /// Fetches and resolves the bin entries for an npm package.
  /// Returns all (bin_name, script_path) pairs.
  async fn resolve_npm_bin_entries(
    &self,
    npm_ref: &NpmPackageReqReference,
  ) -> Result<Option<Vec<(String, String)>>, AnyError> {
    let package_info = self
      .npm_registry_api
      .package_info(&npm_ref.req().name)
      .await?;
    let version_resolver =
      self.npm_version_resolver.get_for_package(&package_info);
    let version_info = version_resolver
      .resolve_best_package_version_info(
        &npm_ref.req().version_req,
        Vec::new().into_iter(),
      )
      .ok();
    let Some(version_info) = version_info else {
      return Ok(None);
    };
    let Some(bin_entries) = version_info.bin.as_ref() else {
      return Ok(None);
    };
    match bin_entries {
      deno_npm::registry::NpmPackageVersionBinEntry::String(script) => {
        let name = &npm_ref.req().name;
        let bin_name = name.rsplit('/').next().unwrap_or(name.as_str());
        Ok(Some(vec![(bin_name.to_string(), script.clone())]))
      }
      deno_npm::registry::NpmPackageVersionBinEntry::Map(data) => Ok(Some(
        data.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
      )),
    }
  }

  async fn resolve_name_from_npm(
    &self,
    npm_ref: &NpmPackageReqReference,
  ) -> Result<Option<String>, AnyError> {
    let entries = self.resolve_npm_bin_entries(npm_ref).await?;
    Ok(Self::infer_name_from_bin_entries(
      entries.as_deref(),
      npm_ref,
    ))
  }

  fn infer_name_from_bin_entries(
    entries: Option<&[(String, String)]>,
    npm_ref: &NpmPackageReqReference,
  ) -> Option<String> {
    let entries = entries?;
    if entries.len() == 1 {
      return Some(entries[0].0.clone());
    }
    if entries.len() > 1 {
      // When there are multiple bin entries, check if one matches the
      // package name (the npm default bin convention). For scoped packages
      // like @scope/name, check the unscoped portion.
      let pkg_name = &npm_ref.req().name;
      let unscoped_name = pkg_name
        .strip_prefix('@')
        .and_then(|s| s.split_once('/'))
        .map(|(_, name)| name)
        .unwrap_or(pkg_name.as_str());
      if entries.iter().any(|(name, _)| name == unscoped_name) {
        return Some(unscoped_name.to_string());
      }
    }
    None
  }

  /// Resolves all bin entries for an npm package URL.
  /// Returns a list of (bin_name, script_path) pairs.
  pub async fn resolve_all_bin_entries_from_npm(
    &self,
    url: &Url,
  ) -> Option<Vec<(String, String)>> {
    let npm_ref = NpmPackageReqReference::from_specifier(url).ok()?;
    self.resolve_npm_bin_entries(&npm_ref).await.ok().flatten()
  }
}

#[cfg(test)]
mod test {
  use std::collections::HashMap;

  use deno_core::url::Url;
  use deno_npm::registry::TestNpmRegistryApi;
  use deno_npm::resolution::NpmVersionResolver;

  use super::BinNameResolver;
  use crate::http_util::HttpClientProvider;

  async fn infer_name_from_url(url: &Url) -> Option<String> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let http_client = HttpClientProvider::new(None, None);
    let registry_api = TestNpmRegistryApi::default();
    registry_api.with_version_info(("@google/gemini-cli", "1.0.0"), |info| {
      info.bin = Some(deno_npm::registry::NpmPackageVersionBinEntry::Map(
        HashMap::from([("gemini".to_string(), "./bin.js".to_string())]),
      ))
    });
    let npm_version_resolver = NpmVersionResolver::default();
    let resolver =
      BinNameResolver::new(&http_client, &registry_api, &npm_version_resolver);
    resolver.infer_name_from_url(url).await
  }

  #[tokio::test]
  async fn install_infer_name_from_url() {
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/abc/server.ts").unwrap()
      )
      .await,
      Some("server".to_string())
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/abc/main.ts").unwrap()
      )
      .await,
      Some("abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/abc/mod.ts").unwrap()
      )
      .await,
      Some("abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/ab%20c/mod.ts").unwrap()
      )
      .await,
      Some("ab c".to_string())
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/abc/index.ts").unwrap()
      )
      .await,
      Some("abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/abc/cli.ts").unwrap()
      )
      .await,
      Some("abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("https://example.com/main.ts").unwrap())
        .await,
      Some("main".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("https://example.com").unwrap()).await,
      None
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("file:///abc/server.ts").unwrap()).await,
      Some("server".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("file:///abc/main.ts").unwrap()).await,
      Some("abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("file:///ab%20c/main.ts").unwrap()).await,
      Some("ab c".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("file:///main.ts").unwrap()).await,
      Some("main".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("file:///").unwrap()).await,
      None
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/abc@0.1.0").unwrap()
      )
      .await,
      Some("abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/abc@0.1.0/main.ts").unwrap()
      )
      .await,
      Some("abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/abc@def@ghi").unwrap()
      )
      .await,
      Some("abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("https://example.com/@abc.ts").unwrap())
        .await,
      Some("@abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(
        &Url::parse("https://example.com/@abc/mod.ts").unwrap()
      )
      .await,
      Some("@abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("file:///@abc.ts").unwrap()).await,
      Some("@abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("file:///@abc/cli.ts").unwrap()).await,
      Some("@abc".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("npm:cowsay@1.2/cowthink").unwrap())
        .await,
      Some("cowthink".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("npm:cowsay@1.2/cowthink/test").unwrap())
        .await,
      Some("cowsay".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("npm:cowsay@1.2").unwrap()).await,
      Some("cowsay".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("npm:@types/node@1.2").unwrap()).await,
      None
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("npm:@slidev/cli@1.2").unwrap()).await,
      Some("slidev".to_string())
    );
    assert_eq!(
      infer_name_from_url(&Url::parse("npm:@google/gemini-cli").unwrap()).await,
      Some("gemini".to_string())
    );
  }

  #[tokio::test]
  async fn install_infer_name_multi_bin_npm() {
    // Package with multiple bins where one matches the package name
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let http_client = HttpClientProvider::new(None, None);
    let registry_api = TestNpmRegistryApi::default();
    registry_api.with_version_info(("pyright", "1.0.0"), |info| {
      info.bin = Some(deno_npm::registry::NpmPackageVersionBinEntry::Map(
        HashMap::from([
          ("pyright".to_string(), "index.js".to_string()),
          (
            "pyright-langserver".to_string(),
            "langserver.index.js".to_string(),
          ),
        ]),
      ))
    });
    let npm_version_resolver = NpmVersionResolver::default();
    let resolver =
      BinNameResolver::new(&http_client, &registry_api, &npm_version_resolver);

    // Should infer "pyright" as the primary name since it matches the package name
    let name = resolver
      .infer_name_from_url(&Url::parse("npm:pyright").unwrap())
      .await;
    assert_eq!(name, Some("pyright".to_string()));
  }

  #[tokio::test]
  async fn install_infer_name_multi_bin_scoped_npm() {
    // Scoped package with multiple bins where one matches the unscoped package name
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let http_client = HttpClientProvider::new(None, None);
    let registry_api = TestNpmRegistryApi::default();
    registry_api.with_version_info(("@denotest/multi-bin", "1.0.0"), |info| {
      info.bin = Some(deno_npm::registry::NpmPackageVersionBinEntry::Map(
        HashMap::from([
          ("multi-bin".to_string(), "./cli.mjs".to_string()),
          ("multi-bin-server".to_string(), "./server.mjs".to_string()),
        ]),
      ))
    });
    let npm_version_resolver = NpmVersionResolver::default();
    let resolver =
      BinNameResolver::new(&http_client, &registry_api, &npm_version_resolver);

    // Should infer "multi-bin" as the primary name
    let name = resolver
      .infer_name_from_url(&Url::parse("npm:@denotest/multi-bin").unwrap())
      .await;
    assert_eq!(name, Some("multi-bin".to_string()));
  }

  #[tokio::test]
  async fn resolve_all_bin_entries_multi_bin() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let http_client = HttpClientProvider::new(None, None);
    let registry_api = TestNpmRegistryApi::default();
    registry_api.with_version_info(("pyright", "1.0.0"), |info| {
      info.bin = Some(deno_npm::registry::NpmPackageVersionBinEntry::Map(
        HashMap::from([
          ("pyright".to_string(), "index.js".to_string()),
          (
            "pyright-langserver".to_string(),
            "langserver.index.js".to_string(),
          ),
        ]),
      ))
    });
    let npm_version_resolver = NpmVersionResolver::default();
    let resolver =
      BinNameResolver::new(&http_client, &registry_api, &npm_version_resolver);

    let entries = resolver
      .resolve_all_bin_entries_from_npm(&Url::parse("npm:pyright").unwrap())
      .await;
    let mut entries = entries.unwrap();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], ("pyright".to_string(), "index.js".to_string()));
    assert_eq!(
      entries[1],
      (
        "pyright-langserver".to_string(),
        "langserver.index.js".to_string()
      )
    );
  }
}
