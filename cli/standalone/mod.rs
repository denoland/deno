// Copyright 2018-2025 the Deno authors. MIT license.

// Allow unused code warnings because we share
// code between the two bin targets.
#![allow(dead_code)]
#![allow(unused_imports)]

use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use binary::StandaloneData;
use binary::StandaloneModules;
use deno_ast::MediaType;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::npm::NpmCacheDir;
use deno_config::workspace::MappedResolution;
use deno_config::workspace::MappedResolutionError;
use deno_config::workspace::ResolverWorkspaceJsrPackage;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::error::ModuleLoaderError;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::v8_set_flags;
use deno_core::FastString;
use deno_core::FeatureChecker;
use deno_core::ModuleLoader;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_core::SourceCodeCacheInfo;
use deno_error::JsErrorBox;
use deno_lib::cache::DenoDirProvider;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::npm::NpmRegistryReadPermissionCheckerMode;
use deno_lib::standalone::virtual_fs::VfsFileSubDataKind;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lib::util::text_encoding::from_utf8_lossy_cow;
use deno_lib::util::text_encoding::from_utf8_lossy_owned;
use deno_lib::util::v8::construct_v8_flags;
use deno_lib::worker::CreateModuleLoaderResult;
use deno_lib::worker::LibMainWorkerFactory;
use deno_lib::worker::LibMainWorkerOptions;
use deno_lib::worker::ModuleLoaderFactory;
use deno_lib::worker::StorageKeyResolver;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_package_json::PackageJsonDepValue;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::npm::managed::ManagedInNpmPkgCheckerCreateOptions;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_resolver::npm::CreateInNpmPkgCheckerOptions;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmReqResolverOptions;
use deno_runtime::deno_fs;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::create_host_defined_options;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_node::RealIsBuiltInNodeModuleChecker;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::WorkerLogLevel;
use deno_semver::npm::NpmPackageReqReference;
use import_map::parse_from_json;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use serialization::DenoCompileModuleSource;
use serialization::SourceMapStore;
use virtual_fs::FileBackedVfs;

use crate::args::create_default_npmrc;
use crate::args::get_root_cert_store;
use crate::args::npm_pkg_req_ref_to_binary_command;
use crate::args::CaData;
use crate::args::NpmInstallDepsProvider;
use crate::cache::Caches;
use crate::cache::NodeAnalysisCache;
use crate::http_util::HttpClientProvider;
use crate::node::CliCjsCodeAnalyzer;
use crate::node::CliNodeCodeTranslator;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::create_npm_process_state_provider;
use crate::npm::CliByonmNpmResolverCreateOptions;
use crate::npm::CliManagedNpmResolverCreateOptions;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverCreateOptions;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::npm::NpmResolutionInitializer;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliNpmReqResolver;
use crate::resolver::NpmModuleLoader;
use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::worker::CliCodeCache;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CliMainWorkerOptions;

pub mod binary;
mod file_system;
mod serialization;
mod virtual_fs;

pub use binary::extract_standalone;
pub use binary::is_standalone_binary;
pub use binary::DenoCompileBinaryWriter;

use self::binary::Metadata;
pub use self::file_system::DenoCompileFileSystem;
