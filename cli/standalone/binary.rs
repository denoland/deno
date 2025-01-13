// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::env;
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
use std::path::Component;
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
use deno_graph::ModuleGraph;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::resolution::SerializedNpmResolutionSnapshotPackage;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_npm::NpmSystemInfo;
use deno_path_util::url_from_directory_path;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_runtime::deno_fs;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_io::fs::FsError;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_semver::npm::NpmVersionReqParseError;
use deno_semver::package::PackageReq;
use deno_semver::Version;
use deno_semver::VersionReqSpecifierParseError;
use deno_telemetry::OtelConfig;
use indexmap::IndexMap;
use log::Level;
use serde::Deserialize;
use serde::Serialize;

use super::file_system::DenoCompileFileSystem;
use super::serialization::deserialize_binary_data_section;
use super::serialization::serialize_binary_data_section;
use super::serialization::DenoCompileModuleData;
use super::serialization::DeserializedDataSection;
use super::serialization::RemoteModulesStore;
use super::serialization::RemoteModulesStoreBuilder;
use super::serialization::SourceMapStore;
use super::virtual_fs::output_vfs;
use super::virtual_fs::BuiltVfs;
use super::virtual_fs::FileBackedVfs;
use super::virtual_fs::FileSystemCaseSensitivity;
use super::virtual_fs::VfsBuilder;
use super::virtual_fs::VfsFileSubDataKind;
use super::virtual_fs::VfsRoot;
use super::virtual_fs::VirtualDirectory;
use super::virtual_fs::VirtualDirectoryEntries;
use super::virtual_fs::WindowsSystemRootablePath;
use crate::args::CaData;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::args::NpmInstallDepsProvider;
use crate::args::PermissionFlags;
use crate::args::UnstableConfig;
use crate::cache::DenoDir;
use crate::cache::FastInsecureHasher;
use crate::emit::Emitter;
use crate::file_fetcher::CliFileFetcher;
use crate::http_util::HttpClientProvider;
use crate::npm::CliNpmResolver;
use crate::npm::InnerCliNpmResolverRef;
use crate::resolver::CjsTracker;
use crate::shared::ReleaseChannel;
use crate::standalone::virtual_fs::VfsEntry;
use crate::util::archive;
use crate::util::fs::canonicalize_path;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

pub static DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME: &str =
  ".deno_compile_node_modules";

/// A URL that can be designated as the base for relative URLs.
///
/// After creation, this URL may be used to get the key for a
/// module in the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandaloneRelativeFileBaseUrl<'a> {
  WindowsSystemRoot,
  Path(&'a Url),
}

impl<'a> StandaloneRelativeFileBaseUrl<'a> {
  /// Gets the module map key of the provided specifier.
  ///
  /// * Descendant file specifiers will be made relative to the base.
  /// * Non-descendant file specifiers will stay as-is (absolute).
  /// * Non-file specifiers will stay as-is.
  pub fn specifier_key<'b>(&self, target: &'b Url) -> Cow<'b, str> {
    if target.scheme() != "file" {
      return Cow::Borrowed(target.as_str());
    }
    let base = match self {
      Self::Path(base) => base,
      Self::WindowsSystemRoot => return Cow::Borrowed(target.path()),
    };

    match base.make_relative(target) {
      Some(relative) => {
        // This is not a great scenario to have because it means that the
        // specifier is outside the vfs and could cause the binary to act
        // strangely. If you encounter this, the fix is to add more paths
        // to the vfs builder by calling `add_possible_min_root_dir`.
        debug_assert!(
          !relative.starts_with("../"),
          "{} -> {} ({})",
          base.as_str(),
          target.as_str(),
          relative,
        );
        Cow::Owned(relative)
      }
      None => Cow::Borrowed(target.as_str()),
    }
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
  pub code_cache_key: Option<u64>,
  pub permissions: PermissionsOptions,
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
  pub otel_config: OtelConfig,
  pub vfs_case_sensitivity: FileSystemCaseSensitivity,
}

#[allow(clippy::too_many_arguments)]
fn write_binary_bytes(
  mut file_writer: File,
  original_bin: Vec<u8>,
  metadata: &Metadata,
  npm_snapshot: Option<SerializedNpmResolutionSnapshot>,
  remote_modules: &RemoteModulesStoreBuilder,
  source_map_store: &SourceMapStore,
  vfs: &BuiltVfs,
  compile_flags: &CompileFlags,
) -> Result<(), AnyError> {
  let data_section_bytes = serialize_binary_data_section(
    metadata,
    npm_snapshot,
    remote_modules,
    source_map_store,
    vfs,
  )
  .context("Serializing binary data section.")?;

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
  pub metadata: Metadata,
  pub modules: StandaloneModules,
  pub npm_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  pub root_path: PathBuf,
  pub source_maps: SourceMapStore,
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
    kind: VfsFileSubDataKind,
  ) -> Result<Option<DenoCompileModuleData<'a>>, AnyError> {
    if specifier.scheme() == "file" {
      let path = deno_path_util::url_to_file_path(specifier)?;
      let bytes = match self.vfs.file_entry(&path) {
        Ok(entry) => self.vfs.read_file_all(entry, kind)?,
        Err(err) if err.kind() == ErrorKind::NotFound => {
          match RealFs.read_file_sync(&path, None) {
            Ok(bytes) => bytes,
            Err(FsError::Io(err)) if err.kind() == ErrorKind::NotFound => {
              return Ok(None)
            }
            Err(err) => return Err(err.into()),
          }
        }
        Err(err) => return Err(err.into()),
      };
      Ok(Some(DenoCompileModuleData {
        media_type: MediaType::from_specifier(specifier),
        specifier,
        data: bytes,
      }))
    } else {
      self.remote_modules.read(specifier).map(|maybe_entry| {
        maybe_entry.map(|entry| DenoCompileModuleData {
          media_type: entry.media_type,
          specifier: entry.specifier,
          data: match kind {
            VfsFileSubDataKind::Raw => entry.data,
            VfsFileSubDataKind::ModuleGraph => {
              entry.transpiled_data.unwrap_or(entry.data)
            }
          },
        })
      })
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
    source_maps,
    vfs_root_entries,
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
    let fs_root = VfsRoot {
      dir: VirtualDirectory {
        // align the name of the directory with the root dir
        name: root_path.file_name().unwrap().to_string_lossy().to_string(),
        entries: vfs_root_entries,
      },
      root_path: root_path.clone(),
      start_file_offset: 0,
    };
    Arc::new(FileBackedVfs::new(
      Cow::Borrowed(vfs_files_data),
      fs_root,
      metadata.vfs_case_sensitivity,
    ))
  };
  Ok(Some(StandaloneData {
    metadata,
    modules: StandaloneModules {
      remote_modules,
      vfs: vfs.clone(),
    },
    npm_snapshot,
    root_path,
    source_maps,
    vfs,
  }))
}

pub struct WriteBinOptions<'a> {
  pub writer: File,
  pub display_output_filename: &'a str,
  pub graph: &'a ModuleGraph,
  pub entrypoint: &'a ModuleSpecifier,
  pub include_files: &'a [ModuleSpecifier],
  pub compile_flags: &'a CompileFlags,
}

pub struct DenoCompileBinaryWriter<'a> {
  cjs_tracker: &'a CjsTracker,
  cli_options: &'a CliOptions,
  deno_dir: &'a DenoDir,
  emitter: &'a Emitter,
  file_fetcher: &'a CliFileFetcher,
  http_client_provider: &'a HttpClientProvider,
  npm_resolver: &'a dyn CliNpmResolver,
  workspace_resolver: &'a WorkspaceResolver,
  npm_system_info: NpmSystemInfo,
}

impl<'a> DenoCompileBinaryWriter<'a> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    cjs_tracker: &'a CjsTracker,
    cli_options: &'a CliOptions,
    deno_dir: &'a DenoDir,
    emitter: &'a Emitter,
    file_fetcher: &'a CliFileFetcher,
    http_client_provider: &'a HttpClientProvider,
    npm_resolver: &'a dyn CliNpmResolver,
    workspace_resolver: &'a WorkspaceResolver,
    npm_system_info: NpmSystemInfo,
  ) -> Self {
    Self {
      cjs_tracker,
      cli_options,
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
    options: WriteBinOptions<'_>,
  ) -> Result<(), AnyError> {
    // Select base binary based on target
    let mut original_binary =
      self.get_base_binary(options.compile_flags).await?;

    if options.compile_flags.no_terminal {
      let target = options.compile_flags.resolve_target();
      if !target.contains("windows") {
        bail!(
          "The `--no-terminal` flag is only available when targeting Windows (current: {})",
          target,
        )
      }
      set_windows_binary_to_gui(&mut original_binary)
        .context("Setting windows binary to GUI.")?;
    }
    if options.compile_flags.icon.is_some() {
      let target = options.compile_flags.resolve_target();
      if !target.contains("windows") {
        bail!(
          "The `--icon` flag is only available when targeting Windows (current: {})",
          target,
        )
      }
    }
    self.write_standalone_binary(options, original_binary)
  }

  async fn get_base_binary(
    &self,
    compile_flags: &CompileFlags,
  ) -> Result<Vec<u8>, AnyError> {
    // Used for testing.
    //
    // Phase 2 of the 'min sized' deno compile RFC talks
    // about adding this as a flag.
    if let Some(path) = get_dev_binary_path() {
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
        .await
        .context("Setting up base binary.")?;
    }

    let read_file = |path: &Path| -> Result<Vec<u8>, AnyError> {
      std::fs::read(path).with_context(|| format!("Reading {}", path.display()))
    };
    let archive_data = read_file(&binary_path)?;
    let temp_dir = tempfile::TempDir::new()?;
    let base_binary_path = archive::unpack_into_dir(archive::UnpackArgs {
      exe_name: "denort",
      archive_name: &binary_name,
      archive_data: &archive_data,
      is_windows: target.contains("windows"),
      dest_path: temp_dir.path(),
    })?;
    let base_binary = read_file(&base_binary_path)?;
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
        bail!("Download could not be found, aborting");
      }
    };

    let create_dir_all = |dir: &Path| {
      std::fs::create_dir_all(dir)
        .with_context(|| format!("Creating {}", dir.display()))
    };
    create_dir_all(output_directory)?;
    let output_path = output_directory.join(binary_path_suffix);
    create_dir_all(output_path.parent().unwrap())?;
    std::fs::write(&output_path, bytes)
      .with_context(|| format!("Writing {}", output_path.display()))?;
    Ok(())
  }

  /// This functions creates a standalone deno binary by appending a bundle
  /// and magic trailer to the currently executing binary.
  #[allow(clippy::too_many_arguments)]
  fn write_standalone_binary(
    &self,
    options: WriteBinOptions<'_>,
    original_bin: Vec<u8>,
  ) -> Result<(), AnyError> {
    let WriteBinOptions {
      writer,
      display_output_filename,
      graph,
      entrypoint,
      include_files,
      compile_flags,
    } = options;
    let ca_data = match self.cli_options.ca_data() {
      Some(CaData::File(ca_file)) => Some(
        std::fs::read(ca_file).with_context(|| format!("Reading {ca_file}"))?,
      ),
      Some(CaData::Bytes(bytes)) => Some(bytes.clone()),
      None => None,
    };
    let mut vfs = VfsBuilder::new();
    let npm_snapshot = match self.npm_resolver.as_inner() {
      InnerCliNpmResolverRef::Managed(managed) => {
        let snapshot =
          managed.serialized_valid_snapshot_for_system(&self.npm_system_info);
        if !snapshot.as_serialized().packages.is_empty() {
          self.fill_npm_vfs(&mut vfs).context("Building npm vfs.")?;
          Some(snapshot)
        } else {
          None
        }
      }
      InnerCliNpmResolverRef::Byonm(_) => {
        self.fill_npm_vfs(&mut vfs)?;
        None
      }
    };
    for include_file in include_files {
      let path = deno_path_util::url_to_file_path(include_file)?;
      vfs
        .add_file_at_path(&path)
        .with_context(|| format!("Including {}", path.display()))?;
    }
    let mut remote_modules_store = RemoteModulesStoreBuilder::default();
    let mut source_maps = Vec::with_capacity(graph.specifiers_count());
    // todo(dsherret): transpile in parallel
    for module in graph.modules() {
      if module.specifier().scheme() == "data" {
        continue; // don't store data urls as an entry as they're in the code
      }
      let (maybe_original_source, maybe_transpiled, media_type) = match module {
        deno_graph::Module::Js(m) => {
          let original_bytes = m.source.as_bytes().to_vec();
          let maybe_transpiled = if m.media_type.is_emittable() {
            let is_cjs = self.cjs_tracker.is_cjs_with_known_is_script(
              &m.specifier,
              m.media_type,
              m.is_script,
            )?;
            let module_kind = ModuleKind::from_is_cjs(is_cjs);
            let (source, source_map) =
              self.emitter.emit_parsed_source_for_deno_compile(
                &m.specifier,
                m.media_type,
                module_kind,
                &m.source,
              )?;
            if source != m.source.as_ref() {
              source_maps.push((&m.specifier, source_map));
              Some(source.into_bytes())
            } else {
              None
            }
          } else {
            None
          };
          (Some(original_bytes), maybe_transpiled, m.media_type)
        }
        deno_graph::Module::Json(m) => {
          (Some(m.source.as_bytes().to_vec()), None, m.media_type)
        }
        deno_graph::Module::Wasm(m) => {
          (Some(m.source.to_vec()), None, MediaType::Wasm)
        }
        deno_graph::Module::Npm(_)
        | deno_graph::Module::Node(_)
        | deno_graph::Module::External(_) => (None, None, MediaType::Unknown),
      };
      if let Some(original_source) = maybe_original_source {
        if module.specifier().scheme() == "file" {
          let file_path = deno_path_util::url_to_file_path(module.specifier())?;
          vfs
            .add_file_with_data(
              &file_path,
              original_source,
              VfsFileSubDataKind::Raw,
            )
            .with_context(|| {
              format!("Failed adding '{}'", file_path.display())
            })?;
          if let Some(transpiled_source) = maybe_transpiled {
            vfs
              .add_file_with_data(
                &file_path,
                transpiled_source,
                VfsFileSubDataKind::ModuleGraph,
              )
              .with_context(|| {
                format!("Failed adding '{}'", file_path.display())
              })?;
          }
        } else {
          remote_modules_store.add(
            module.specifier(),
            media_type,
            original_source,
            maybe_transpiled,
          );
        }
      }
    }
    remote_modules_store.add_redirects(&graph.redirects);

    if let Some(import_map) = self.workspace_resolver.maybe_import_map() {
      if let Ok(file_path) = url_to_file_path(import_map.base_url()) {
        if let Some(import_map_parent_dir) = file_path.parent() {
          // tell the vfs about the import map's parent directory in case it
          // falls outside what the root of where the VFS will be based
          vfs.add_possible_min_root_dir(import_map_parent_dir);
        }
      }
    }
    if let Some(node_modules_dir) = self.npm_resolver.root_node_modules_path() {
      // ensure the vfs doesn't go below the node_modules directory's parent
      if let Some(parent) = node_modules_dir.parent() {
        vfs.add_possible_min_root_dir(parent);
      }
    }

    let vfs = self.build_vfs_consolidating_global_npm_cache(vfs);
    let root_dir_url = match &vfs.root_path {
      WindowsSystemRootablePath::Path(dir) => {
        Some(url_from_directory_path(dir)?)
      }
      WindowsSystemRootablePath::WindowSystemRoot => None,
    };
    let root_dir_url = match &root_dir_url {
      Some(url) => StandaloneRelativeFileBaseUrl::Path(url),
      None => StandaloneRelativeFileBaseUrl::WindowsSystemRoot,
    };

    let code_cache_key = if self.cli_options.code_cache_enabled() {
      let mut hasher = FastInsecureHasher::new_deno_versioned();
      for module in graph.modules() {
        if let Some(source) = module.source() {
          hasher
            .write(root_dir_url.specifier_key(module.specifier()).as_bytes());
          hasher.write(source.as_bytes());
        }
      }
      Some(hasher.finish())
    } else {
      None
    };

    let mut source_map_store = SourceMapStore::with_capacity(source_maps.len());
    for (specifier, source_map) in source_maps {
      source_map_store.add(
        Cow::Owned(root_dir_url.specifier_key(specifier).into_owned()),
        Cow::Owned(source_map.into_bytes()),
      );
    }

    let node_modules = match self.npm_resolver.as_inner() {
      InnerCliNpmResolverRef::Managed(_) => {
        npm_snapshot.as_ref().map(|_| NodeModules::Managed {
          node_modules_dir: self.npm_resolver.root_node_modules_path().map(
            |path| {
              root_dir_url
                .specifier_key(
                  &ModuleSpecifier::from_directory_path(path).unwrap(),
                )
                .into_owned()
            },
          ),
        })
      }
      InnerCliNpmResolverRef::Byonm(resolver) => Some(NodeModules::Byonm {
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
    };

    let env_vars_from_env_file = match self.cli_options.env_file_name() {
      Some(env_filenames) => {
        let mut aggregated_env_vars = IndexMap::new();
        for env_filename in env_filenames.iter().rev() {
          log::info!("{} Environment variables from the file \"{}\" were embedded in the generated executable file", crate::colors::yellow("Warning"), env_filename);

          let env_vars = get_file_env_vars(env_filename.to_string())?;
          aggregated_env_vars.extend(env_vars);
        }
        aggregated_env_vars
      }
      None => Default::default(),
    };

    output_vfs(&vfs, display_output_filename);

    let metadata = Metadata {
      argv: compile_flags.args.clone(),
      seed: self.cli_options.seed(),
      code_cache_key,
      location: self.cli_options.location_flag().clone(),
      permissions: self.cli_options.permissions_options(),
      v8_flags: self.cli_options.v8_flags().clone(),
      unsafely_ignore_certificate_errors: self
        .cli_options
        .unsafely_ignore_certificate_errors()
        .clone(),
      log_level: self.cli_options.log_level(),
      ca_stores: self.cli_options.ca_stores().clone(),
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
        bare_node_builtins: self.cli_options.unstable_bare_node_builtins(),
        detect_cjs: self.cli_options.unstable_detect_cjs(),
        sloppy_imports: self.cli_options.unstable_sloppy_imports(),
        features: self.cli_options.unstable_features(),
        npm_lazy_caching: self.cli_options.unstable_npm_lazy_caching(),
      },
      otel_config: self.cli_options.otel_config(),
      vfs_case_sensitivity: vfs.case_sensitivity,
    };

    write_binary_bytes(
      writer,
      original_bin,
      &metadata,
      npm_snapshot.map(|s| s.into_serialized()),
      &remote_modules_store,
      &source_map_store,
      &vfs,
      compile_flags,
    )
    .context("Writing binary bytes")
  }

  fn fill_npm_vfs(&self, builder: &mut VfsBuilder) -> Result<(), AnyError> {
    fn maybe_warn_different_system(system_info: &NpmSystemInfo) {
      if system_info != &NpmSystemInfo::default() {
        log::warn!("{} The node_modules directory may be incompatible with the target system.", crate::colors::yellow("Warning"));
      }
    }

    match self.npm_resolver.as_inner() {
      InnerCliNpmResolverRef::Managed(npm_resolver) => {
        if let Some(node_modules_path) = npm_resolver.root_node_modules_path() {
          maybe_warn_different_system(&self.npm_system_info);
          builder.add_dir_recursive(node_modules_path)?;
          Ok(())
        } else {
          // we'll flatten to remove any custom registries later
          let mut packages =
            npm_resolver.all_system_packages(&self.npm_system_info);
          packages.sort_by(|a, b| a.id.cmp(&b.id)); // determinism
          for package in packages {
            let folder =
              npm_resolver.resolve_pkg_folder_from_pkg_id(&package.id)?;
            builder.add_dir_recursive(&folder)?;
          }
          Ok(())
        }
      }
      InnerCliNpmResolverRef::Byonm(_) => {
        maybe_warn_different_system(&self.npm_system_info);
        for pkg_json in self.cli_options.workspace().package_jsons() {
          builder.add_file_at_path(&pkg_json.path)?;
        }
        // traverse and add all the node_modules directories in the workspace
        let mut pending_dirs = VecDeque::new();
        pending_dirs.push_back(
          self
            .cli_options
            .workspace()
            .root_dir()
            .to_file_path()
            .unwrap(),
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
        Ok(())
      }
    }
  }

  fn build_vfs_consolidating_global_npm_cache(
    &self,
    mut vfs: VfsBuilder,
  ) -> BuiltVfs {
    match self.npm_resolver.as_inner() {
      InnerCliNpmResolverRef::Managed(npm_resolver) => {
        if npm_resolver.root_node_modules_path().is_some() {
          return vfs.build();
        }

        let global_cache_root_path = npm_resolver.global_cache_root_path();

        // Flatten all the registries folders into a single ".deno_compile_node_modules/localhost" folder
        // that will be used by denort when loading the npm cache. This avoids us exposing
        // the user's private registry information and means we don't have to bother
        // serializing all the different registry config into the binary.
        let Some(root_dir) = vfs.get_dir_mut(global_cache_root_path) else {
          return vfs.build();
        };

        root_dir.name = DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME.to_string();
        let mut new_entries = Vec::with_capacity(root_dir.entries.len());
        let mut localhost_entries = IndexMap::new();
        for entry in root_dir.entries.take_inner() {
          match entry {
            VfsEntry::Dir(mut dir) => {
              for entry in dir.entries.take_inner() {
                log::debug!("Flattening {} into node_modules", entry.name());
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
          entries: VirtualDirectoryEntries::new(
            localhost_entries.into_iter().map(|(_, v)| v).collect(),
          ),
        }));
        root_dir.entries = VirtualDirectoryEntries::new(new_entries);

        // it's better to not expose the user's cache directory, so take it out
        // of there
        let case_sensitivity = vfs.case_sensitivity();
        let parent = global_cache_root_path.parent().unwrap();
        let parent_dir = vfs.get_dir_mut(parent).unwrap();
        let index = parent_dir
          .entries
          .binary_search(
            DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME,
            case_sensitivity,
          )
          .unwrap();
        let npm_global_cache_dir_entry = parent_dir.entries.remove(index);

        // go up from the ancestors removing empty directories...
        // this is not as optimized as it could be
        let mut last_name =
          Cow::Borrowed(DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME);
        for ancestor in parent.ancestors() {
          let dir = vfs.get_dir_mut(ancestor).unwrap();
          if let Ok(index) =
            dir.entries.binary_search(&last_name, case_sensitivity)
          {
            dir.entries.remove(index);
          }
          last_name = Cow::Owned(dir.name.clone());
          if !dir.entries.is_empty() {
            break;
          }
        }

        // now build the vfs and add the global cache dir entry there
        let mut built_vfs = vfs.build();
        built_vfs
          .entries
          .insert(npm_global_cache_dir_entry, case_sensitivity);
        built_vfs
      }
      InnerCliNpmResolverRef::Byonm(_) => vfs.build(),
    }
  }
}

fn get_denort_path(deno_exe: PathBuf) -> Option<OsString> {
  let mut denort = deno_exe;
  denort.set_file_name(if cfg!(windows) {
    "denort.exe"
  } else {
    "denort"
  });
  denort.exists().then(|| denort.into_os_string())
}

fn get_dev_binary_path() -> Option<OsString> {
  env::var_os("DENORT_BIN").or_else(|| {
    env::current_exe().ok().and_then(|exec_path| {
      if exec_path
        .components()
        .any(|component| component == Component::Normal("target".as_ref()))
      {
        get_denort_path(exec_path)
      } else {
        None
      }
    })
  })
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
