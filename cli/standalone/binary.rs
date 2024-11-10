// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::env::current_exe;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::future::Future;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_ast::ModuleSpecifier;
use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::ResolverWorkspaceJsrPackage;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::io::AllowStdIo;
use deno_core::futures::AsyncReadExt;
use deno_core::futures::AsyncSeekExt;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::source::RealFileSystem;
use deno_graph::ModuleGraph;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::resolution::SerializedNpmResolutionSnapshotPackage;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_io::fs::FsError;
use deno_runtime::deno_node::PackageJson;
use deno_semver::npm::NpmVersionReqParseError;
use deno_semver::package::PackageReq;
use deno_semver::Version;
use deno_semver::VersionReqSpecifierParseError;
use indexmap::IndexMap;
use log::Level;
use serde::Deserialize;
use serde::Serialize;

use crate::args::CaData;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::args::NpmInstallDepsProvider;
use crate::args::PermissionFlags;
use crate::args::UnstableConfig;
use crate::cache::DenoDir;
use crate::emit::Emitter;
use crate::file_fetcher::FileFetcher;
use crate::http_util::HttpClientProvider;
use crate::npm::CliNpmResolver;
use crate::npm::InnerCliNpmResolverRef;
use crate::resolver::CjsTracker;
use crate::shared::ReleaseChannel;
use crate::standalone::virtual_fs::VfsEntry;
use crate::util::archive;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

use super::file_system::DenoCompileFileSystem;
use super::serialization::deserialize_binary_data_section;
use super::serialization::serialize_binary_data_section;
use super::serialization::DenoCompileModuleData;
use super::serialization::DeserializedDataSection;
use super::serialization::RemoteModulesStore;
use super::serialization::RemoteModulesStoreBuilder;
use super::virtual_fs::FileBackedVfs;
use super::virtual_fs::VfsBuilder;
use super::virtual_fs::VfsRoot;
use super::virtual_fs::VirtualDirectory;

/// A URL that can be designated as the base for relative URLs.
///
/// After creation, this URL may be used to get the key for a
/// module in the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StandaloneRelativeFileBaseUrl<'a>(&'a Url);

impl<'a> From<&'a Url> for StandaloneRelativeFileBaseUrl<'a> {
  fn from(url: &'a Url) -> Self {
    Self(url)
  }
}

impl<'a> StandaloneRelativeFileBaseUrl<'a> {
  pub fn new(url: &'a Url) -> Self {
    debug_assert_eq!(url.scheme(), "file");
    Self(url)
  }

  /// Gets the module map key of the provided specifier.
  ///
  /// * Descendant file specifiers will be made relative to the base.
  /// * Non-descendant file specifiers will stay as-is (absolute).
  /// * Non-file specifiers will stay as-is.
  pub fn specifier_key<'b>(&self, target: &'b Url) -> Cow<'b, str> {
    if target.scheme() != "file" {
      return Cow::Borrowed(target.as_str());
    }

    match self.0.make_relative(target) {
      Some(relative) => {
        if relative.starts_with("../") {
          Cow::Borrowed(target.as_str())
        } else {
          Cow::Owned(relative)
        }
      }
      None => Cow::Borrowed(target.as_str()),
    }
  }

  pub fn inner(&self) -> &Url {
    self.0
  }
}

#[derive(Deserialize, Serialize)]
pub enum NodeModules {
  Managed {
    /// Relative path for the node_modules directory in the vfs.
    node_modules_dir: Option<String>,
  },
  Byonm {
    root_node_modules_dir: Option<String>,
  },
}

#[derive(Deserialize, Serialize)]
pub struct SerializedWorkspaceResolverImportMap {
  pub specifier: String,
  pub json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerializedResolverWorkspaceJsrPackage {
  pub relative_base: String,
  pub name: String,
  pub version: Option<Version>,
  pub exports: IndexMap<String, String>,
}

#[derive(Deserialize, Serialize)]
pub struct SerializedWorkspaceResolver {
  pub import_map: Option<SerializedWorkspaceResolverImportMap>,
  pub jsr_pkgs: Vec<SerializedResolverWorkspaceJsrPackage>,
  pub package_jsons: BTreeMap<String, serde_json::Value>,
  pub pkg_json_resolution: PackageJsonDepResolution,
}

// Note: Don't use hashmaps/hashsets. Ensure the serialization
// is deterministic.
#[derive(Deserialize, Serialize)]
pub struct Metadata {
  pub argv: Vec<String>,
  pub seed: Option<u64>,
  pub permissions: PermissionFlags,
  pub location: Option<Url>,
  pub v8_flags: Vec<String>,
  pub log_level: Option<Level>,
  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<Vec<u8>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub env_vars_from_env_file: IndexMap<String, String>,
  pub workspace_resolver: SerializedWorkspaceResolver,
  pub entrypoint_key: String,
  pub node_modules: Option<NodeModules>,
  pub unstable_config: UnstableConfig,
}

fn write_binary_bytes(
  mut file_writer: File,
  original_bin: Vec<u8>,
  metadata: &Metadata,
  npm_snapshot: Option<SerializedNpmResolutionSnapshot>,
  remote_modules: &RemoteModulesStoreBuilder,
  vfs: VfsBuilder,
  compile_flags: &CompileFlags,
) -> Result<(), AnyError> {
  let data_section_bytes =
    serialize_binary_data_section(metadata, npm_snapshot, remote_modules, vfs)?;

  let target = compile_flags.resolve_target();
  if target.contains("linux") {
    libsui::Elf::new(&original_bin).append(
      "d3n0l4nd",
      &data_section_bytes,
      &mut file_writer,
    )?;
  } else if target.contains("windows") {
    let mut pe = libsui::PortableExecutable::from(&original_bin)?;
    if let Some(icon) = compile_flags.icon.as_ref() {
      let icon = std::fs::read(icon)?;
      pe = pe.set_icon(&icon)?;
    }

    pe.write_resource("d3n0l4nd", data_section_bytes)?
      .build(&mut file_writer)?;
  } else if target.contains("darwin") {
    libsui::Macho::from(original_bin)?
      .write_section("d3n0l4nd", data_section_bytes)?
      .build_and_sign(&mut file_writer)?;
  }
  Ok(())
}

pub fn is_standalone_binary(exe_path: &Path) -> bool {
  let Ok(data) = std::fs::read(exe_path) else {
    return false;
  };

  libsui::utils::is_elf(&data)
    || libsui::utils::is_pe(&data)
    || libsui::utils::is_macho(&data)
}

pub struct StandaloneData {
  pub fs: Arc<dyn deno_fs::FileSystem>,
  pub metadata: Metadata,
  pub modules: StandaloneModules,
  pub npm_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  pub root_path: PathBuf,
  pub vfs: Arc<FileBackedVfs>,
}

pub struct StandaloneModules {
  remote_modules: RemoteModulesStore,
  vfs: Arc<FileBackedVfs>,
}

impl StandaloneModules {
  pub fn resolve_specifier<'a>(
    &'a self,
    specifier: &'a ModuleSpecifier,
  ) -> Result<Option<&'a ModuleSpecifier>, AnyError> {
    if specifier.scheme() == "file" {
      Ok(Some(specifier))
    } else {
      self.remote_modules.resolve_specifier(specifier)
    }
  }

  pub fn has_file(&self, path: &Path) -> bool {
    self.vfs.file_entry(path).is_ok()
  }

  pub fn read<'a>(
    &'a self,
    specifier: &'a ModuleSpecifier,
  ) -> Result<Option<DenoCompileModuleData<'a>>, AnyError> {
    if specifier.scheme() == "file" {
      let path = deno_path_util::url_to_file_path(specifier)?;
      let bytes = match self.vfs.file_entry(&path) {
        Ok(entry) => self.vfs.read_file_all(entry)?,
        Err(err) if err.kind() == ErrorKind::NotFound => {
          let bytes = match RealFs.read_file_sync(&path, None) {
            Ok(bytes) => bytes,
            Err(FsError::Io(err)) if err.kind() == ErrorKind::NotFound => {
              return Ok(None)
            }
            Err(err) => return Err(err.into()),
          };
          Cow::Owned(bytes)
        }
        Err(err) => return Err(err.into()),
      };
      Ok(Some(DenoCompileModuleData {
        media_type: MediaType::from_specifier(specifier),
        specifier,
        data: bytes,
      }))
    } else {
      self.remote_modules.read(specifier)
    }
  }
}

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a standalone
/// binary by skipping over the trailer width at the end of the file,
/// then checking for the magic trailer string `d3n0l4nd`. If found,
/// the bundle is executed. If not, this function exits with `Ok(None)`.
pub fn extract_standalone(
  cli_args: Cow<Vec<OsString>>,
) -> Result<Option<StandaloneData>, AnyError> {
  let Some(data) = libsui::find_section("d3n0l4nd") else {
    return Ok(None);
  };

  let DeserializedDataSection {
    mut metadata,
    npm_snapshot,
    remote_modules,
    mut vfs_dir,
    vfs_files_data,
  } = match deserialize_binary_data_section(data)? {
    Some(data_section) => data_section,
    None => return Ok(None),
  };

  let root_path = {
    let maybe_current_exe = std::env::current_exe().ok();
    let current_exe_name = maybe_current_exe
      .as_ref()
      .and_then(|p| p.file_name())
      .map(|p| p.to_string_lossy())
      // should never happen
      .unwrap_or_else(|| Cow::Borrowed("binary"));
    std::env::temp_dir().join(format!("deno-compile-{}", current_exe_name))
  };
  let cli_args = cli_args.into_owned();
  metadata.argv.reserve(cli_args.len() - 1);
  for arg in cli_args.into_iter().skip(1) {
    metadata.argv.push(arg.into_string().unwrap());
  }
  let vfs = {
    // align the name of the directory with the root dir
    vfs_dir.name = root_path.file_name().unwrap().to_string_lossy().to_string();

    let fs_root = VfsRoot {
      dir: vfs_dir,
      root_path: root_path.clone(),
      start_file_offset: 0,
    };
    Arc::new(FileBackedVfs::new(Cow::Borrowed(vfs_files_data), fs_root))
  };
  let fs: Arc<dyn deno_fs::FileSystem> =
    Arc::new(DenoCompileFileSystem::new(vfs.clone()));
  Ok(Some(StandaloneData {
    fs,
    metadata,
    modules: StandaloneModules {
      remote_modules,
      vfs: vfs.clone(),
    },
    npm_snapshot,
    root_path,
    vfs,
  }))
}

pub struct DenoCompileBinaryWriter<'a> {
  cjs_tracker: &'a CjsTracker,
  deno_dir: &'a DenoDir,
  emitter: &'a Emitter,
  file_fetcher: &'a FileFetcher,
  http_client_provider: &'a HttpClientProvider,
  npm_resolver: &'a dyn CliNpmResolver,
  workspace_resolver: &'a WorkspaceResolver,
  npm_system_info: NpmSystemInfo,
}

impl<'a> DenoCompileBinaryWriter<'a> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    cjs_tracker: &'a CjsTracker,
    deno_dir: &'a DenoDir,
    emitter: &'a Emitter,
    file_fetcher: &'a FileFetcher,
    http_client_provider: &'a HttpClientProvider,
    npm_resolver: &'a dyn CliNpmResolver,
    workspace_resolver: &'a WorkspaceResolver,
    npm_system_info: NpmSystemInfo,
  ) -> Self {
    Self {
      cjs_tracker,
      deno_dir,
      emitter,
      file_fetcher,
      http_client_provider,
      npm_resolver,
      workspace_resolver,
      npm_system_info,
    }
  }

  pub async fn write_bin(
    &self,
    writer: File,
    graph: &ModuleGraph,
    root_dir_url: StandaloneRelativeFileBaseUrl<'_>,
    entrypoint: &ModuleSpecifier,
    compile_flags: &CompileFlags,
    cli_options: &CliOptions,
  ) -> Result<(), AnyError> {
    // Select base binary based on target
    let mut original_binary = self.get_base_binary(compile_flags).await?;

    if compile_flags.no_terminal {
      let target = compile_flags.resolve_target();
      if !target.contains("windows") {
        bail!(
          "The `--no-terminal` flag is only available when targeting Windows (current: {})",
          target,
        )
      }
      set_windows_binary_to_gui(&mut original_binary)?;
    }
    if compile_flags.icon.is_some() {
      let target = compile_flags.resolve_target();
      if !target.contains("windows") {
        bail!(
          "The `--icon` flag is only available when targeting Windows (current: {})",
          target,
        )
      }
    }
    self
      .write_standalone_binary(
        writer,
        original_binary,
        graph,
        root_dir_url,
        entrypoint,
        cli_options,
        compile_flags,
      )
      .await
  }

  async fn get_base_binary(
    &self,
    compile_flags: &CompileFlags,
  ) -> Result<Vec<u8>, AnyError> {
    // Used for testing.
    //
    // Phase 2 of the 'min sized' deno compile RFC talks
    // about adding this as a flag.
    if let Some(path) = std::env::var_os("DENORT_BIN") {
      return std::fs::read(&path).with_context(|| {
        format!("Could not find denort at '{}'", path.to_string_lossy())
      });
    }

    let target = compile_flags.resolve_target();
    let binary_name = format!("denort-{target}.zip");

    let binary_path_suffix =
      match crate::version::DENO_VERSION_INFO.release_channel {
        ReleaseChannel::Canary => {
          format!(
            "canary/{}/{}",
            crate::version::DENO_VERSION_INFO.git_hash,
            binary_name
          )
        }
        _ => {
          format!("release/v{}/{}", env!("CARGO_PKG_VERSION"), binary_name)
        }
      };

    let download_directory = self.deno_dir.dl_folder_path();
    let binary_path = download_directory.join(&binary_path_suffix);

    if !binary_path.exists() {
      self
        .download_base_binary(&download_directory, &binary_path_suffix)
        .await?;
    }

    let archive_data = std::fs::read(binary_path)?;
    let temp_dir = tempfile::TempDir::new()?;
    let base_binary_path = archive::unpack_into_dir(archive::UnpackArgs {
      exe_name: "denort",
      archive_name: &binary_name,
      archive_data: &archive_data,
      is_windows: target.contains("windows"),
      dest_path: temp_dir.path(),
    })?;
    let base_binary = std::fs::read(base_binary_path)?;
    drop(temp_dir); // delete the temp dir
    Ok(base_binary)
  }

  async fn download_base_binary(
    &self,
    output_directory: &Path,
    binary_path_suffix: &str,
  ) -> Result<(), AnyError> {
    let download_url = format!("https://dl.deno.land/{binary_path_suffix}");
    let maybe_bytes = {
      let progress_bars = ProgressBar::new(ProgressBarStyle::DownloadBars);
      let progress = progress_bars.update(&download_url);

      self
        .http_client_provider
        .get_or_create()?
        .download_with_progress_and_retries(
          download_url.parse()?,
          None,
          &progress,
        )
        .await?
    };
    let bytes = match maybe_bytes {
      Some(bytes) => bytes,
      None => {
        log::info!("Download could not be found, aborting");
        std::process::exit(1)
      }
    };

    std::fs::create_dir_all(output_directory)?;
    let output_path = output_directory.join(binary_path_suffix);
    std::fs::create_dir_all(output_path.parent().unwrap())?;
    tokio::fs::write(output_path, bytes).await?;
    Ok(())
  }

  /// This functions creates a standalone deno binary by appending a bundle
  /// and magic trailer to the currently executing binary.
  #[allow(clippy::too_many_arguments)]
  async fn write_standalone_binary(
    &self,
    writer: File,
    original_bin: Vec<u8>,
    graph: &ModuleGraph,
    root_dir_url: StandaloneRelativeFileBaseUrl<'_>,
    entrypoint: &ModuleSpecifier,
    cli_options: &CliOptions,
    compile_flags: &CompileFlags,
  ) -> Result<(), AnyError> {
    let ca_data = match cli_options.ca_data() {
      Some(CaData::File(ca_file)) => Some(
        std::fs::read(ca_file)
          .with_context(|| format!("Reading: {ca_file}"))?,
      ),
      Some(CaData::Bytes(bytes)) => Some(bytes.clone()),
      None => None,
    };
    let root_path = root_dir_url.inner().to_file_path().unwrap();
    let (maybe_npm_vfs, node_modules, npm_snapshot) = match self
      .npm_resolver
      .as_inner()
    {
      InnerCliNpmResolverRef::Managed(managed) => {
        let snapshot =
          managed.serialized_valid_snapshot_for_system(&self.npm_system_info);
        if !snapshot.as_serialized().packages.is_empty() {
          let npm_vfs_builder = self.build_npm_vfs(&root_path, cli_options)?;
          (
            Some(npm_vfs_builder),
            Some(NodeModules::Managed {
              node_modules_dir: self.npm_resolver.root_node_modules_path().map(
                |path| {
                  root_dir_url
                    .specifier_key(
                      &ModuleSpecifier::from_directory_path(path).unwrap(),
                    )
                    .into_owned()
                },
              ),
            }),
            Some(snapshot),
          )
        } else {
          (None, None, None)
        }
      }
      InnerCliNpmResolverRef::Byonm(resolver) => {
        let npm_vfs_builder = self.build_npm_vfs(&root_path, cli_options)?;
        (
          Some(npm_vfs_builder),
          Some(NodeModules::Byonm {
            root_node_modules_dir: resolver.root_node_modules_path().map(
              |node_modules_dir| {
                root_dir_url
                  .specifier_key(
                    &ModuleSpecifier::from_directory_path(node_modules_dir)
                      .unwrap(),
                  )
                  .into_owned()
              },
            ),
          }),
          None,
        )
      }
    };
    let mut vfs = if let Some(npm_vfs) = maybe_npm_vfs {
      npm_vfs
    } else {
      VfsBuilder::new(root_path.clone())?
    };
    let mut remote_modules_store = RemoteModulesStoreBuilder::default();
    for module in graph.modules() {
      if module.specifier().scheme() == "data" {
        continue; // don't store data urls as an entry as they're in the code
      }
      let (maybe_source, media_type) = match module {
        deno_graph::Module::Js(m) => {
          let source = if m.media_type.is_emittable() {
            let is_cjs = self.cjs_tracker.is_cjs_with_known_is_script(
              &m.specifier,
              m.media_type,
              m.is_script,
            )?;
            let module_kind = ModuleKind::from_is_cjs(is_cjs);
            let source = self
              .emitter
              .emit_parsed_source(
                &m.specifier,
                m.media_type,
                module_kind,
                &m.source,
              )
              .await?;
            source.into_bytes()
          } else {
            m.source.as_bytes().to_vec()
          };
          (Some(source), m.media_type)
        }
        deno_graph::Module::Json(m) => {
          (Some(m.source.as_bytes().to_vec()), m.media_type)
        }
        deno_graph::Module::Npm(_)
        | deno_graph::Module::Node(_)
        | deno_graph::Module::External(_) => (None, MediaType::Unknown),
      };
      if module.specifier().scheme() == "file" {
        let file_path = deno_path_util::url_to_file_path(module.specifier())?;
        vfs
          .add_file_with_data(
            &file_path,
            match maybe_source {
              Some(source) => source,
              None => RealFs.read_file_sync(&file_path, None)?,
            },
          )
          .with_context(|| {
            format!("Failed adding '{}'", file_path.display())
          })?;
      } else if let Some(source) = maybe_source {
        remote_modules_store.add(module.specifier(), media_type, source);
      }
    }
    remote_modules_store.add_redirects(&graph.redirects);

    let env_vars_from_env_file = match cli_options.env_file_name() {
      Some(env_filename) => {
        log::info!("{} Environment variables from the file \"{}\" were embedded in the generated executable file", crate::colors::yellow("Warning"), env_filename);
        get_file_env_vars(env_filename.to_string())?
      }
      None => Default::default(),
    };

    let metadata = Metadata {
      argv: compile_flags.args.clone(),
      seed: cli_options.seed(),
      location: cli_options.location_flag().clone(),
      permissions: cli_options.permission_flags().clone(),
      v8_flags: cli_options.v8_flags().clone(),
      unsafely_ignore_certificate_errors: cli_options
        .unsafely_ignore_certificate_errors()
        .clone(),
      log_level: cli_options.log_level(),
      ca_stores: cli_options.ca_stores().clone(),
      ca_data,
      env_vars_from_env_file,
      entrypoint_key: root_dir_url.specifier_key(entrypoint).into_owned(),
      workspace_resolver: SerializedWorkspaceResolver {
        import_map: self.workspace_resolver.maybe_import_map().map(|i| {
          SerializedWorkspaceResolverImportMap {
            specifier: if i.base_url().scheme() == "file" {
              root_dir_url.specifier_key(i.base_url()).into_owned()
            } else {
              // just make a remote url local
              "deno.json".to_string()
            },
            json: i.to_json(),
          }
        }),
        jsr_pkgs: self
          .workspace_resolver
          .jsr_packages()
          .map(|pkg| SerializedResolverWorkspaceJsrPackage {
            relative_base: root_dir_url.specifier_key(&pkg.base).into_owned(),
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            exports: pkg.exports.clone(),
          })
          .collect(),
        package_jsons: self
          .workspace_resolver
          .package_jsons()
          .map(|pkg_json| {
            (
              root_dir_url
                .specifier_key(&pkg_json.specifier())
                .into_owned(),
              serde_json::to_value(pkg_json).unwrap(),
            )
          })
          .collect(),
        pkg_json_resolution: self.workspace_resolver.pkg_json_dep_resolution(),
      },
      node_modules,
      unstable_config: UnstableConfig {
        legacy_flag_enabled: false,
        bare_node_builtins: cli_options.unstable_bare_node_builtins(),
        detect_cjs: cli_options.unstable_detect_cjs(),
        sloppy_imports: cli_options.unstable_sloppy_imports(),
        features: cli_options.unstable_features(),
      },
    };

    write_binary_bytes(
      writer,
      original_bin,
      &metadata,
      npm_snapshot.map(|s| s.into_serialized()),
      &remote_modules_store,
      vfs,
      compile_flags,
    )
  }

  fn build_npm_vfs(
    &self,
    root_path: &Path,
    cli_options: &CliOptions,
  ) -> Result<VfsBuilder, AnyError> {
    fn maybe_warn_different_system(system_info: &NpmSystemInfo) {
      if system_info != &NpmSystemInfo::default() {
        log::warn!("{} The node_modules directory may be incompatible with the target system.", crate::colors::yellow("Warning"));
      }
    }

    match self.npm_resolver.as_inner() {
      InnerCliNpmResolverRef::Managed(npm_resolver) => {
        if let Some(node_modules_path) = npm_resolver.root_node_modules_path() {
          maybe_warn_different_system(&self.npm_system_info);
          let mut builder = VfsBuilder::new(root_path.to_path_buf())?;
          builder.add_dir_recursive(node_modules_path)?;
          Ok(builder)
        } else {
          // DO NOT include the user's registry url as it may contain credentials,
          // but also don't make this dependent on the registry url
          let global_cache_root_path = npm_resolver.global_cache_root_path();
          let mut builder =
            VfsBuilder::new(global_cache_root_path.to_path_buf())?;
          let mut packages =
            npm_resolver.all_system_packages(&self.npm_system_info);
          packages.sort_by(|a, b| a.id.cmp(&b.id)); // determinism
          for package in packages {
            let folder =
              npm_resolver.resolve_pkg_folder_from_pkg_id(&package.id)?;
            builder.add_dir_recursive(&folder)?;
          }

          // Flatten all the registries folders into a single ".deno_compile_node_modules/localhost" folder
          // that will be used by denort when loading the npm cache. This avoids us exposing
          // the user's private registry information and means we don't have to bother
          // serializing all the different registry config into the binary.
          builder.with_root_dir(|root_dir| {
            root_dir.name = ".deno_compile_node_modules".to_string();
            let mut new_entries = Vec::with_capacity(root_dir.entries.len());
            let mut localhost_entries = IndexMap::new();
            for entry in std::mem::take(&mut root_dir.entries) {
              match entry {
                VfsEntry::Dir(dir) => {
                  for entry in dir.entries {
                    log::debug!(
                      "Flattening {} into node_modules",
                      entry.name()
                    );
                    if let Some(existing) =
                      localhost_entries.insert(entry.name().to_string(), entry)
                    {
                      panic!(
                        "Unhandled scenario where a duplicate entry was found: {:?}",
                        existing
                      );
                    }
                  }
                }
                VfsEntry::File(_) | VfsEntry::Symlink(_) => {
                  new_entries.push(entry);
                }
              }
            }
            new_entries.push(VfsEntry::Dir(VirtualDirectory {
              name: "localhost".to_string(),
              entries: localhost_entries.into_iter().map(|(_, v)| v).collect(),
            }));
            // needs to be sorted by name
            new_entries.sort_by(|a, b| a.name().cmp(b.name()));
            root_dir.entries = new_entries;
          });

          builder.set_new_root_path(root_path.to_path_buf())?;

          Ok(builder)
        }
      }
      InnerCliNpmResolverRef::Byonm(_) => {
        maybe_warn_different_system(&self.npm_system_info);
        let mut builder = VfsBuilder::new(root_path.to_path_buf())?;
        for pkg_json in cli_options.workspace().package_jsons() {
          builder.add_file_at_path(&pkg_json.path)?;
        }
        // traverse and add all the node_modules directories in the workspace
        let mut pending_dirs = VecDeque::new();
        pending_dirs.push_back(
          cli_options.workspace().root_dir().to_file_path().unwrap(),
        );
        while let Some(pending_dir) = pending_dirs.pop_front() {
          let mut entries = fs::read_dir(&pending_dir)
            .with_context(|| {
              format!("Failed reading: {}", pending_dir.display())
            })?
            .collect::<Result<Vec<_>, _>>()?;
          entries.sort_by_cached_key(|entry| entry.file_name()); // determinism
          for entry in entries {
            let path = entry.path();
            if !path.is_dir() {
              continue;
            }
            if path.ends_with("node_modules") {
              builder.add_dir_recursive(&path)?;
            } else {
              pending_dirs.push_back(path);
            }
          }
        }
        Ok(builder)
      }
    }
  }
}

/// This function returns the environment variables specified
/// in the passed environment file.
fn get_file_env_vars(
  filename: String,
) -> Result<IndexMap<String, String>, dotenvy::Error> {
  let mut file_env_vars = IndexMap::new();
  for item in dotenvy::from_filename_iter(filename)? {
    let Ok((key, val)) = item else {
      continue; // this failure will be warned about on load
    };
    file_env_vars.insert(key, val);
  }
  Ok(file_env_vars)
}

/// This function sets the subsystem field in the PE header to 2 (GUI subsystem)
/// For more information about the PE header: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
fn set_windows_binary_to_gui(bin: &mut [u8]) -> Result<(), AnyError> {
  // Get the PE header offset located in an i32 found at offset 60
  // See: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#ms-dos-stub-image-only
  let start_pe = u32::from_le_bytes((bin[60..64]).try_into()?);

  // Get image type (PE32 or PE32+) indicates whether the binary is 32 or 64 bit
  // The used offset and size values can be found here:
  // https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#optional-header-image-only
  let start_32 = start_pe as usize + 28;
  let magic_32 =
    u16::from_le_bytes(bin[(start_32)..(start_32 + 2)].try_into()?);

  let start_64 = start_pe as usize + 24;
  let magic_64 =
    u16::from_le_bytes(bin[(start_64)..(start_64 + 2)].try_into()?);

  // Take the standard fields size for the current architecture (32 or 64 bit)
  // This is the ofset for the Windows-Specific fields
  let standard_fields_size = if magic_32 == 0x10b {
    28
  } else if magic_64 == 0x20b {
    24
  } else {
    bail!("Could not find a matching magic field in the PE header")
  };

  // Set the subsystem field (offset 68) to 2 (GUI subsystem)
  // For all possible options, see: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#optional-header-windows-specific-fields-image-only
  let subsystem_offset = 68;
  let subsystem_start =
    start_pe as usize + standard_fields_size + subsystem_offset;
  let subsystem: u16 = 2;
  bin[(subsystem_start)..(subsystem_start + 2)]
    .copy_from_slice(&subsystem.to_le_bytes());
  Ok(())
}
