// Copyright 2018-2025 the Deno authors. MIT license.

use deno_npm_cache::NpmCacheHttpClient;

use crate::NpmInstaller;

// todo(https://github.com/rust-lang/rust/issues/109737): remove once_cell after get_or_try_init is stabilized
type Deferred<T> = once_cell::sync::OnceCell<T>;

pub trait NpmInstallFactorySys: crate::NpmInstallerSys {}

pub struct NpmInstallFactory<
  TNpmCacheHttpClient: NpmCacheHttpClient,
  TSys: NpmInstallFactorySys,
> {
  npm_installer: Deferred<NpmInstaller<TNpmCacheHttpClient, TSys>>,
  sys: TSys,
}

impl<TNpmCacheHttpClient: NpmCacheHttpClient, TSys: NpmInstallFactorySys>
  NpmInstallFactory<TNpmCacheHttpClient, TSys>
{
}
