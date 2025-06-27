// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_error::JsErrorBox;
use deno_graph::NpmLoadError;
use deno_graph::NpmResolvePkgReqsResult;
use deno_npm::resolution::NpmResolutionError;
use deno_npm_cache::NpmCacheHttpClient;
use deno_resolver::graph::FoundPackageJsonDepFlag;
use deno_semver::package::PackageReq;

use crate::NpmInstaller;
use crate::NpmInstallerSys;
use crate::PackageCaching;

#[derive(Debug, Clone, Copy)]
pub enum NpmCachingStrategy {
  Eager,
  Lazy,
  Manual,
}

#[derive(Debug)]
pub struct NpmDenoGraphResolver<
  TNpmCacheHttpClient: NpmCacheHttpClient,
  TSys: NpmInstallerSys,
> {
  npm_installer: Option<Arc<NpmInstaller<TNpmCacheHttpClient, TSys>>>,
  found_package_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
  npm_caching: NpmCachingStrategy,
}

impl<TNpmCacheHttpClient: NpmCacheHttpClient, TSys: NpmInstallerSys>
  NpmDenoGraphResolver<TNpmCacheHttpClient, TSys>
{
  pub fn new(
    npm_installer: Option<Arc<NpmInstaller<TNpmCacheHttpClient, TSys>>>,
    found_package_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
    npm_caching: NpmCachingStrategy,
  ) -> Self {
    Self {
      npm_installer,
      found_package_json_dep_flag,
      npm_caching,
    }
  }
}

#[async_trait::async_trait(?Send)]
impl<TNpmCacheHttpClient: NpmCacheHttpClient, TSys: NpmInstallerSys>
  deno_graph::source::NpmResolver
  for NpmDenoGraphResolver<TNpmCacheHttpClient, TSys>
{
  fn load_and_cache_npm_package_info(&self, package_name: &str) {
    // ok not to do this in Wasm because this is just an optimization
    #[cfg(target_arch = "wasm32")]
    {
      _ = package_name;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
      if let Some(npm_installer) = &self.npm_installer {
        let npm_installer = npm_installer.clone();
        let package_name = package_name.to_string();
        deno_unsync::spawn(async move {
          let _ignore = npm_installer.cache_package_info(&package_name).await;
        });
      }
    }
  }

  async fn resolve_pkg_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> NpmResolvePkgReqsResult {
    match &self.npm_installer {
      Some(npm_installer) => {
        let top_level_result = if self.found_package_json_dep_flag.is_raised() {
          npm_installer
            .ensure_top_level_package_json_install()
            .await
            .map(|_| ())
        } else {
          Ok(())
        };

        let result = npm_installer
          .add_package_reqs_raw(
            package_reqs,
            match self.npm_caching {
              NpmCachingStrategy::Eager => Some(PackageCaching::All),
              NpmCachingStrategy::Lazy => {
                Some(PackageCaching::Only(package_reqs.into()))
              }
              NpmCachingStrategy::Manual => None,
            },
          )
          .await;

        NpmResolvePkgReqsResult {
          results: result
            .results
            .into_iter()
            .map(|r| {
              r.map_err(|err| match err {
                NpmResolutionError::Registry(e) => {
                  NpmLoadError::RegistryInfo(Arc::new(e))
                }
                NpmResolutionError::Resolution(e) => {
                  NpmLoadError::PackageReqResolution(Arc::new(e))
                }
                NpmResolutionError::DependencyEntry(e) => {
                  NpmLoadError::PackageReqResolution(Arc::new(e))
                }
              })
            })
            .collect(),
          dep_graph_result: match top_level_result {
            Ok(()) => result
              .dependencies_result
              .map_err(|e| Arc::new(e) as Arc<dyn deno_error::JsErrorClass>),
            Err(err) => Err(Arc::new(err)),
          },
        }
      }
      None => {
        let err = Arc::new(JsErrorBox::generic(
          "npm specifiers were requested; but --no-npm is specified",
        ));
        NpmResolvePkgReqsResult {
          results: package_reqs
            .iter()
            .map(|_| Err(NpmLoadError::RegistryInfo(err.clone())))
            .collect(),
          dep_graph_result: Err(err),
        }
      }
    }
  }
}
