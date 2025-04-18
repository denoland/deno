// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use capacity_builder::BytesAppendable;
use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_ast::ModuleSpecifier;
use deno_cache_dir::CACHE_PERM;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::ModuleGraph;
use deno_lib::args::CaData;
use deno_lib::args::UnstableConfig;
use deno_lib::shared::ReleaseChannel;
use deno_lib::standalone::binary::CjsExportAnalysisEntry;
use deno_lib::standalone::binary::Metadata;
use deno_lib::standalone::binary::NodeModules;
use deno_lib::standalone::binary::RemoteModuleEntry;
use deno_lib::standalone::binary::SerializedResolverWorkspaceJsrPackage;
use deno_lib::standalone::binary::SerializedWorkspaceResolver;
use deno_lib::standalone::binary::SerializedWorkspaceResolverImportMap;
use deno_lib::standalone::binary::SpecifierDataStore;
use deno_lib::standalone::binary::SpecifierId;
use deno_lib::standalone::binary::MAGIC_BYTES;
use deno_lib::standalone::virtual_fs::BuiltVfs;
use deno_lib::standalone::virtual_fs::VfsBuilder;
use deno_lib::standalone::virtual_fs::VfsEntry;
use deno_lib::standalone::virtual_fs::VirtualDirectory;
use deno_lib::standalone::virtual_fs::VirtualDirectoryEntries;
use deno_lib::standalone::virtual_fs::WindowsSystemRootablePath;
use deno_lib::standalone::virtual_fs::DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lib::version::DENO_VERSION_INFO;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::NpmSystemInfo;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_path_util::url_from_directory_path;
use deno_path_util::url_to_file_path;
use deno_resolver::workspace::WorkspaceResolver;
use indexmap::IndexMap;
use node_resolver::analyze::ResolvedCjsAnalysis;

use super::virtual_fs::output_vfs;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::cache::DenoDir;
use crate::emit::Emitter;
use crate::http_util::HttpClientProvider;
use crate::node::CliCjsModuleExportAnalyzer;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::sys::CliSys;
use crate::util::archive;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

/// A URL that can be designated as the base for relative URLs.
///
/// After creation, this URL may be used to get the key for a
/// module in the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandaloneRelativeFileBaseUrl<'a> {
  WindowsSystemRoot,
  Path(&'a Url),
}

impl StandaloneRelativeFileBaseUrl<'_> {
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

struct SpecifierStore<'a> {
  data: IndexMap<&'a Url, SpecifierId>,
}

impl<'a> SpecifierStore<'a> {
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      data: IndexMap::with_capacity(capacity),
    }
  }

  pub fn get_or_add(&mut self, specifier: &'a Url) -> SpecifierId {
    let len = self.data.len();
    let entry = self.data.entry(specifier);
    match entry {
      indexmap::map::Entry::Occupied(occupied_entry) => *occupied_entry.get(),
      indexmap::map::Entry::Vacant(vacant_entry) => {
        let new_id = SpecifierId::new(len as u32);
        vacant_entry.insert(new_id);
        new_id
      }
    }
  }

  pub fn for_serialization(
    self,
    base_url: &StandaloneRelativeFileBaseUrl<'a>,
  ) -> SpecifierStoreForSerialization<'a> {
    SpecifierStoreForSerialization {
      data: self
        .data
        .into_iter()
        .map(|(specifier, id)| (base_url.specifier_key(specifier), id))
        .collect(),
    }
  }
}

struct SpecifierStoreForSerialization<'a> {
  data: Vec<(Cow<'a, str>, SpecifierId)>,
}

impl<'a> BytesAppendable<'a> for &'a SpecifierStoreForSerialization<'a> {
  fn append_to_builder<TBytes: capacity_builder::BytesType>(
    self,
    builder: &mut capacity_builder::BytesBuilder<'a, TBytes>,
  ) {
    builder.append_le(self.data.len() as u32);
    for (specifier_str, id) in &self.data {
      builder.append_le(specifier_str.len() as u32);
      builder.append(specifier_str.as_ref());
      builder.append(*id);
    }
  }
}

pub fn is_standalone_binary(exe_path: &Path) -> bool {
  let Ok(data) = std::fs::read(exe_path) else {
    return false;
  };

  libsui::utils::is_elf(&data)
    || libsui::utils::is_pe(&data)
    || libsui::utils::is_macho(&data)
}

pub struct WriteBinOptions<'a> {
  pub writer: File,
  pub display_output_filename: &'a str,
  pub graph: &'a ModuleGraph,
  pub entrypoint: &'a ModuleSpecifier,
  pub include_paths: &'a [ModuleSpecifier],
  pub exclude_paths: Vec<PathBuf>,
  pub compile_flags: &'a CompileFlags,
}

pub struct DenoCompileBinaryWriter<'a> {
  cjs_module_export_analyzer: &'a CliCjsModuleExportAnalyzer,
  cjs_tracker: &'a CliCjsTracker,
  cli_options: &'a CliOptions,
  deno_dir: &'a DenoDir,
  emitter: &'a Emitter,
  http_client_provider: &'a HttpClientProvider,
  npm_resolver: &'a CliNpmResolver,
  workspace_resolver: &'a WorkspaceResolver<CliSys>,
  npm_system_info: NpmSystemInfo,
}

impl<'a> DenoCompileBinaryWriter<'a> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    cjs_module_export_analyzer: &'a CliCjsModuleExportAnalyzer,
    cjs_tracker: &'a CliCjsTracker,
    cli_options: &'a CliOptions,
    deno_dir: &'a DenoDir,
    emitter: &'a Emitter,
    http_client_provider: &'a HttpClientProvider,
    npm_resolver: &'a CliNpmResolver,
    workspace_resolver: &'a WorkspaceResolver<CliSys>,
    npm_system_info: NpmSystemInfo,
  ) -> Self {
    Self {
      cjs_module_export_analyzer,
      cjs_tracker,
      cli_options,
      deno_dir,
      emitter,
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
    self.write_standalone_binary(options, original_binary).await
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

    let binary_path_suffix = match DENO_VERSION_INFO.release_channel {
      ReleaseChannel::Canary => {
        format!("canary/{}/{}", DENO_VERSION_INFO.git_hash, binary_name)
      }
      _ => {
        format!("release/v{}/{}", DENO_VERSION_INFO.deno, binary_name)
      }
    };

    let download_directory = self.deno_dir.dl_folder_path();
    let binary_path = download_directory.join(&binary_path_suffix);

    let read_file = |path: &Path| -> Result<Vec<u8>, AnyError> {
      std::fs::read(path).with_context(|| format!("Reading {}", path.display()))
    };
    let archive_data = if binary_path.exists() {
      read_file(&binary_path)?
    } else {
      self
        .download_base_binary(&binary_path, &binary_path_suffix)
        .await
        .context("Setting up base binary.")?
    };
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
    output_path: &Path,
    binary_path_suffix: &str,
  ) -> Result<Vec<u8>, AnyError> {
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
    create_dir_all(output_path.parent().unwrap())?;
    atomic_write_file_with_retries(
      &CliSys::default(),
      output_path,
      &bytes,
      CACHE_PERM,
    )
    .with_context(|| format!("Writing {}", output_path.display()))?;
    Ok(bytes)
  }

  /// This functions creates a standalone deno binary by appending a bundle
  /// and magic trailer to the currently executing binary.
  #[allow(clippy::too_many_arguments)]
  async fn write_standalone_binary(
    &self,
    options: WriteBinOptions<'_>,
    original_bin: Vec<u8>,
  ) -> Result<(), AnyError> {
    let WriteBinOptions {
      writer,
      display_output_filename,
      graph,
      entrypoint,
      include_paths,
      exclude_paths,
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
    for path in exclude_paths {
      vfs.add_exclude_path(path);
    }
    let npm_snapshot = match &self.npm_resolver {
      CliNpmResolver::Managed(managed) => {
        let snapshot = managed
          .resolution()
          .serialized_valid_snapshot_for_system(&self.npm_system_info);
        if !snapshot.as_serialized().packages.is_empty() {
          self.fill_npm_vfs(&mut vfs).context("Building npm vfs.")?;
          Some(snapshot)
        } else {
          None
        }
      }
      CliNpmResolver::Byonm(_) => {
        self.fill_npm_vfs(&mut vfs)?;
        None
      }
    };
    for include_file in include_paths {
      let path = deno_path_util::url_to_file_path(include_file)?;
      vfs
        .add_path(&path)
        .with_context(|| format!("Including {}", path.display()))?;
    }
    let specifiers_count = graph.specifiers_count();
    let mut specifier_store = SpecifierStore::with_capacity(specifiers_count);
    let mut remote_modules_store =
      SpecifierDataStore::with_capacity(specifiers_count);
    // todo(dsherret): transpile and analyze CJS in parallel
    for module in graph.modules() {
      if module.specifier().scheme() == "data" {
        continue; // don't store data urls as an entry as they're in the code
      }
      let mut maybe_source_map = None;
      let mut maybe_transpiled = None;
      let mut maybe_cjs_analysis = None;
      let (maybe_original_source, media_type) = match module {
        deno_graph::Module::Js(m) => {
          let specifier = &m.specifier;
          let original_bytes = m.source.as_bytes();
          if self.cjs_tracker.is_maybe_cjs(specifier, m.media_type)? {
            if self.cjs_tracker.is_cjs_with_known_is_script(
              specifier,
              m.media_type,
              m.is_script,
            )? {
              let cjs_analysis = self
                .cjs_module_export_analyzer
                .analyze_all_exports(
                  module.specifier(),
                  Some(Cow::Borrowed(m.source.as_ref())),
                )
                .await?;
              maybe_cjs_analysis = Some(match cjs_analysis {
                ResolvedCjsAnalysis::Esm(_) => CjsExportAnalysisEntry::Esm,
                ResolvedCjsAnalysis::Cjs(exports) => {
                  CjsExportAnalysisEntry::Cjs(
                    exports.into_iter().collect::<Vec<_>>(),
                  )
                }
              });
            } else {
              maybe_cjs_analysis = Some(CjsExportAnalysisEntry::Esm);
            }
          }
          if m.media_type.is_emittable() {
            let module_kind = match maybe_cjs_analysis.as_ref() {
              Some(CjsExportAnalysisEntry::Cjs(_)) => ModuleKind::Cjs,
              _ => ModuleKind::Esm,
            };
            let (source, source_map) =
              self.emitter.emit_parsed_source_for_deno_compile(
                &m.specifier,
                m.media_type,
                module_kind,
                &m.source,
              )?;
            if source != m.source.as_ref() {
              maybe_source_map = Some(source_map.into_bytes());
              maybe_transpiled = Some(source.into_bytes());
            }
          }
          (Some(original_bytes), m.media_type)
        }
        deno_graph::Module::Json(m) => {
          (Some(m.source.as_bytes()), m.media_type)
        }
        deno_graph::Module::Wasm(m) => {
          (Some(m.source.as_ref()), MediaType::Wasm)
        }
        deno_graph::Module::Npm(_)
        | deno_graph::Module::Node(_)
        | deno_graph::Module::External(_) => (None, MediaType::Unknown),
      };
      if let Some(original_source) = maybe_original_source {
        let maybe_cjs_export_analysis = maybe_cjs_analysis
          .as_ref()
          .map(bincode::serialize)
          .transpose()?;
        if module.specifier().scheme() == "file" {
          let file_path = deno_path_util::url_to_file_path(module.specifier())?;
          vfs
            .add_file_with_data(
              &file_path,
              deno_lib::standalone::virtual_fs::AddFileDataOptions {
                data: original_source.to_vec(),
                maybe_transpiled,
                maybe_source_map,
                maybe_cjs_export_analysis,
                mtime: file_path
                  .metadata()
                  .ok()
                  .and_then(|m| m.modified().ok()),
              },
            )
            .with_context(|| {
              format!("Failed adding '{}'", file_path.display())
            })?;
        } else {
          let specifier_id = specifier_store.get_or_add(module.specifier());
          remote_modules_store.add(
            specifier_id,
            RemoteModuleEntry {
              media_type,
              data: Cow::Borrowed(original_source),
              maybe_transpiled: maybe_transpiled.map(Cow::Owned),
              maybe_source_map: maybe_source_map.map(Cow::Owned),
              maybe_cjs_export_analysis: maybe_cjs_export_analysis
                .map(Cow::Owned),
            },
          );
        }
      }
    }

    let mut redirects_store =
      SpecifierDataStore::with_capacity(graph.redirects.len());
    for (from, to) in &graph.redirects {
      redirects_store.add(
        specifier_store.get_or_add(from),
        specifier_store.get_or_add(to),
      );
    }

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

    // do CJS export analysis on all the files in the VFS
    // todo(dsherret): analyze cjs in parallel
    let mut to_add = Vec::new();
    for (file_path, file) in vfs.iter_files() {
      if file.cjs_export_analysis_offset.is_some() {
        continue; // already analyzed
      }
      let specifier = deno_path_util::url_from_file_path(&file_path)?;
      let media_type = MediaType::from_specifier(&specifier);
      if self.cjs_tracker.is_maybe_cjs(&specifier, media_type)? {
        let maybe_source = vfs
          .file_bytes(file.offset)
          .map(|text| String::from_utf8_lossy(text));
        let cjs_analysis_result = self
          .cjs_module_export_analyzer
          .analyze_all_exports(&specifier, maybe_source)
          .await;
        let analysis = match cjs_analysis_result {
          Ok(ResolvedCjsAnalysis::Esm(_)) => CjsExportAnalysisEntry::Esm,
          Ok(ResolvedCjsAnalysis::Cjs(exports)) => {
            CjsExportAnalysisEntry::Cjs(exports.into_iter().collect::<Vec<_>>())
          }
          Err(err) => {
            log::debug!(
              "Had cjs export analysis error for '{}': {}",
              specifier,
              err
            );
            CjsExportAnalysisEntry::Error(err.to_string())
          }
        };
        to_add.push((file_path, bincode::serialize(&analysis)?));
      }
    }
    for (file_path, analysis) in to_add {
      vfs.add_cjs_export_analysis(&file_path, analysis);
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

    let node_modules = match &self.npm_resolver {
      CliNpmResolver::Managed(_) => {
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
      CliNpmResolver::Byonm(resolver) => Some(NodeModules::Byonm {
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
        features: self.cli_options.unstable_features(),
        lazy_dynamic_imports: self.cli_options.unstable_lazy_dynamic_imports(),
        npm_lazy_caching: self.cli_options.unstable_npm_lazy_caching(),
        sloppy_imports: self.cli_options.unstable_sloppy_imports(),
      },
      otel_config: self.cli_options.otel_config(),
      vfs_case_sensitivity: vfs.case_sensitivity,
    };

    let (data_section_bytes, section_sizes) = serialize_binary_data_section(
      &metadata,
      npm_snapshot.map(|s| s.into_serialized()),
      &specifier_store.for_serialization(&root_dir_url),
      &redirects_store,
      &remote_modules_store,
      &vfs,
    )
    .context("Serializing binary data section.")?;

    log::info!(
      "\n{} {}",
      crate::colors::bold("Files:"),
      crate::util::display::human_size(section_sizes.vfs as f64)
    );
    log::info!(
      "{} {}",
      crate::colors::bold("Metadata:"),
      crate::util::display::human_size(section_sizes.metadata as f64)
    );
    log::info!(
      "{} {}\n",
      crate::colors::bold("Remote modules:"),
      crate::util::display::human_size(section_sizes.remote_modules as f64)
    );

    write_binary_bytes(writer, original_bin, data_section_bytes, compile_flags)
      .context("Writing binary bytes")
  }

  fn fill_npm_vfs(&self, builder: &mut VfsBuilder) -> Result<(), AnyError> {
    fn maybe_warn_different_system(system_info: &NpmSystemInfo) {
      if system_info != &NpmSystemInfo::default() {
        log::warn!("{} The node_modules directory may be incompatible with the target system.", crate::colors::yellow("Warning"));
      }
    }

    match &self.npm_resolver {
      CliNpmResolver::Managed(npm_resolver) => {
        if let Some(node_modules_path) = npm_resolver.root_node_modules_path() {
          maybe_warn_different_system(&self.npm_system_info);
          builder.add_dir_recursive(node_modules_path)?;
          Ok(())
        } else {
          // we'll flatten to remove any custom registries later
          let mut packages = npm_resolver
            .resolution()
            .all_system_packages(&self.npm_system_info);
          packages.sort_by(|a, b| a.id.cmp(&b.id)); // determinism
          for package in packages {
            let folder =
              npm_resolver.resolve_pkg_folder_from_pkg_id(&package.id)?;
            builder.add_dir_recursive(&folder)?;
          }
          Ok(())
        }
      }
      CliNpmResolver::Byonm(_) => {
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
    match &self.npm_resolver {
      CliNpmResolver::Managed(npm_resolver) => {
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
        for ancestor in
          parent.ancestors().map(Some).chain(std::iter::once(None))
        {
          let dir = if let Some(ancestor) = ancestor {
            vfs.get_dir_mut(ancestor).unwrap()
          } else if cfg!(windows) {
            vfs.get_system_root_dir_mut()
          } else {
            break;
          };
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
      CliNpmResolver::Byonm(_) => vfs.build(),
    }
  }
}

#[allow(clippy::too_many_arguments)]
fn write_binary_bytes(
  mut file_writer: File,
  original_bin: Vec<u8>,
  data_section_bytes: Vec<u8>,
  compile_flags: &CompileFlags,
) -> Result<(), AnyError> {
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

struct BinaryDataSectionSizes {
  metadata: usize,
  remote_modules: usize,
  vfs: usize,
}

/// Binary format:
/// * d3n0l4nd
/// * <metadata_len><metadata>
/// * <npm_snapshot_len><npm_snapshot>
/// * <specifiers>
/// * <redirects>
/// * <remote_modules>
/// * <vfs_headers_len><vfs_headers>
/// * <vfs_file_data_len><vfs_file_data>
/// * d3n0l4nd
#[allow(clippy::too_many_arguments)]
fn serialize_binary_data_section(
  metadata: &Metadata,
  npm_snapshot: Option<SerializedNpmResolutionSnapshot>,
  specifiers: &SpecifierStoreForSerialization,
  redirects: &SpecifierDataStore<SpecifierId>,
  remote_modules: &SpecifierDataStore<RemoteModuleEntry<'_>>,
  vfs: &BuiltVfs,
) -> Result<(Vec<u8>, BinaryDataSectionSizes), AnyError> {
  let metadata = serde_json::to_string(metadata)?;
  let npm_snapshot =
    npm_snapshot.map(serialize_npm_snapshot).unwrap_or_default();
  let serialized_vfs = serde_json::to_string(&vfs.entries)?;

  let remote_modules_len = Cell::new(0);
  let metadata_len = Cell::new(0);
  let vfs_len = Cell::new(0);

  let bytes = capacity_builder::BytesBuilder::build(|builder| {
    builder.append(MAGIC_BYTES);
    // 1. Metadata
    {
      builder.append_le(metadata.len() as u64);
      builder.append(&metadata);
    }
    // 2. Npm snapshot
    {
      builder.append_le(npm_snapshot.len() as u64);
      builder.append(&npm_snapshot);
    }
    metadata_len.set(builder.len());
    // 3. Specifiers
    builder.append(specifiers);
    // 4. Redirects
    redirects.serialize(builder);
    // 5. Remote modules
    remote_modules.serialize(builder);
    remote_modules_len.set(builder.len() - metadata_len.get());
    // 6. VFS
    {
      builder.append_le(serialized_vfs.len() as u64);
      builder.append(&serialized_vfs);
      let vfs_bytes_len = vfs.files.iter().map(|f| f.len() as u64).sum::<u64>();
      builder.append_le(vfs_bytes_len);
      for file in &vfs.files {
        builder.append(file);
      }
    }
    vfs_len.set(builder.len() - remote_modules_len.get());

    // write the magic bytes at the end so we can use it
    // to make sure we've deserialized correctly
    builder.append(MAGIC_BYTES);
  })?;

  Ok((
    bytes,
    BinaryDataSectionSizes {
      metadata: metadata_len.get(),
      remote_modules: remote_modules_len.get(),
      vfs: vfs_len.get(),
    },
  ))
}

fn serialize_npm_snapshot(
  mut snapshot: SerializedNpmResolutionSnapshot,
) -> Vec<u8> {
  fn append_string(bytes: &mut Vec<u8>, string: &str) {
    let len = string.len() as u32;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(string.as_bytes());
  }

  snapshot.packages.sort_by(|a, b| a.id.cmp(&b.id)); // determinism
  let ids_to_stored_ids = snapshot
    .packages
    .iter()
    .enumerate()
    .map(|(i, pkg)| (&pkg.id, i as u32))
    .collect::<HashMap<_, _>>();

  let mut root_packages: Vec<_> = snapshot.root_packages.iter().collect();
  root_packages.sort();
  let mut bytes = Vec::new();

  bytes.extend_from_slice(&(snapshot.packages.len() as u32).to_le_bytes());
  for pkg in &snapshot.packages {
    append_string(&mut bytes, &pkg.id.as_serialized());
  }

  bytes.extend_from_slice(&(root_packages.len() as u32).to_le_bytes());
  for (req, id) in root_packages {
    append_string(&mut bytes, &req.to_string());
    let id = ids_to_stored_ids.get(&id).unwrap();
    bytes.extend_from_slice(&id.to_le_bytes());
  }

  for pkg in &snapshot.packages {
    let deps_len = pkg.dependencies.len() as u32;
    bytes.extend_from_slice(&deps_len.to_le_bytes());
    let mut deps: Vec<_> = pkg.dependencies.iter().collect();
    deps.sort();
    for (req, id) in deps {
      append_string(&mut bytes, req);
      let id = ids_to_stored_ids.get(&id).unwrap();
      bytes.extend_from_slice(&id.to_le_bytes());
    }
  }

  bytes
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
