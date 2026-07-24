// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::collections::HashMap;
use std::collections::HashSet;
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
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::ModuleGraph;
use deno_lib::args::CaData;
use deno_lib::args::UnstableConfig;
use deno_lib::shared::ReleaseChannel;
use deno_lib::standalone::binary::CjsExportAnalysisEntry;
use deno_lib::standalone::binary::MAGIC_BYTES;
use deno_lib::standalone::binary::Metadata;
use deno_lib::standalone::binary::NodeModules;
use deno_lib::standalone::binary::RemoteModuleEntry;
use deno_lib::standalone::binary::SerializedResolverWorkspaceJsrPackage;
use deno_lib::standalone::binary::SerializedWorkspaceResolver;
use deno_lib::standalone::binary::SerializedWorkspaceResolverImportMap;
use deno_lib::standalone::binary::SpecifierDataStore;
use deno_lib::standalone::binary::SpecifierId;
use deno_lib::standalone::virtual_fs::BuiltVfs;
use deno_lib::standalone::virtual_fs::DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME;
use deno_lib::standalone::virtual_fs::VfsBuilder;
use deno_lib::standalone::virtual_fs::VfsEntry;
use deno_lib::standalone::virtual_fs::VirtualDirectory;
use deno_lib::standalone::virtual_fs::VirtualDirectoryEntries;
use deno_lib::standalone::virtual_fs::WindowsSystemRootablePath;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lib::util::text_encoding::is_valid_utf8;
use deno_lib::util::v8::construct_v8_flags;
use deno_lib::version::DENO_VERSION_INFO;
use deno_npm::NpmSystemInfo;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_path_util::url_from_directory_path;
use deno_path_util::url_to_file_path;
use deno_resolver::file_fetcher::FetchLocalOptions;
use deno_resolver::file_fetcher::FetchOptions;
use deno_resolver::file_fetcher::FetchPermissionsOptionRef;
use deno_resolver::workspace::WorkspaceResolver;
use deno_semver::npm::NpmPackageReqReference;
use indexmap::IndexMap;
use node_resolver::analyze::ResolvedCjsAnalysis;

use super::virtual_fs::output_vfs;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::args::CompileFlagsExt;
use crate::args::get_default_v8_flags;
use crate::cache::DenoDir;
use crate::file_fetcher::CliFileFetcher;
use crate::http_util::HttpClientProvider;
use crate::module_loader::CliEmitter;
use crate::node::CliCjsModuleExportAnalyzer;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::sys::CliSys;
use crate::util::archive;
use crate::util::env::handle_dotenv_error;
use crate::util::env::handle_dotenv_io_error;
use crate::util::env::handle_dotenv_not_found;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::progress_bar::ProgressMessagePrompt;

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

/// Given a canonical npm package folder (e.g.
/// `<.deno>/<id>/node_modules/@scope/name`), walk up to the enclosing
/// `node_modules/` directory. Embedding from there picks up sibling
/// symlinks the deno linker creates for direct dependencies, which the
/// canonical folder itself doesn't contain.
fn pkg_folder_node_modules_root(folder: &Path) -> Option<&Path> {
  let mut current = folder.parent()?;
  loop {
    if current.file_name() == Some(std::ffi::OsStr::new("node_modules")) {
      return Some(current);
    }
    current = current.parent()?;
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

/// Validate a user-provided `--app-name`. The name becomes a single directory
/// component under the platform's app data directory at runtime. Because a
/// binary can be cross-compiled, validate against the union of what every
/// target OS allows so the baked identity resolves to one unambiguous
/// directory component everywhere, rather than escaping the directory or
/// failing on the target's filesystem. Done here so the user gets a clear
/// compile-time error instead of a surprising (or unusable) store location.
fn validate_app_name(app_name: &str) -> Result<(), AnyError> {
  const RESERVED_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6",
    "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6",
    "lpt7", "lpt8", "lpt9",
  ];

  // Windows reserved device names match case-insensitively against the portion
  // before the first `.` (so `nul`, `NUL`, and `nul.txt` all match).
  let stem = app_name.split('.').next().unwrap_or(app_name);
  let is_reserved_name =
    RESERVED_NAMES.iter().any(|n| stem.eq_ignore_ascii_case(n));

  let reason = if app_name.is_empty() {
    Some("must not be empty")
  } else if app_name == "." || app_name == ".." {
    Some("must not be `.` or `..`")
  } else if app_name
    // `/` and `\` are path separators; `<>:"|?*` are reserved on Windows;
    // control characters are rejected by the filesystem.
    .contains(|c: char| {
      matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*')
        || c.is_control()
    })
  {
    Some("must not contain path separators or any of `<>:\"|?*`")
  } else if app_name.ends_with('.')
    || app_name.ends_with(' ')
    || app_name.starts_with(' ')
  {
    // Windows silently strips trailing dots and spaces, which would change the
    // identity out from under the user; a leading space is an error-prone
    // directory name everywhere, so reject it too.
    Some("must not start or end with a space, or end with a `.`")
  } else if is_reserved_name {
    Some("must not be a reserved device name (e.g. `CON`, `NUL`, `COM1`)")
  } else {
    None
  };

  if let Some(reason) = reason {
    bail!("Invalid `--app-name` value {:?}: {}.", app_name, reason);
  }
  Ok(())
}

fn default_app_name(display_output_filename: &str, is_desktop: bool) -> String {
  if is_desktop {
    // A desktop build's compile output is an intermediate shared library. The
    // final app is packaged without this platform-specific extension, so keep
    // its baked identity (and default window title) in sync with that name.
    Path::new(display_output_filename)
      .file_stem()
      .and_then(|stem| stem.to_str())
      .unwrap_or(display_output_filename)
      .to_string()
  } else {
    display_output_filename
      .strip_suffix(".exe")
      .unwrap_or(display_output_filename)
      .to_string()
  }
}

/// Resolve the stable app identity baked into a compiled binary: an explicit
/// `--app-name`, otherwise the output file name (minus the executable extension
/// added by Deno). The derived default is held to the same rules as an explicit
/// flag, since it becomes a single directory component at runtime (possibly on
/// a different target OS when cross-compiling); otherwise an output name like
/// `aux` or one with a trailing dot would silently break persistent storage on
/// the target.
fn resolve_app_name(
  compile_flags: &CompileFlags,
  display_output_filename: &str,
  is_desktop: bool,
) -> Result<String, AnyError> {
  let app_name = compile_flags
    .app_name
    .clone()
    .unwrap_or_else(|| default_app_name(display_output_filename, is_desktop));
  validate_app_name(&app_name)?;
  Ok(app_name)
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
  emitter: &'a CliEmitter,
  file_fetcher: &'a CliFileFetcher,
  http_client_provider: &'a HttpClientProvider,
  npm_resolver: &'a CliNpmResolver,
  workspace_resolver: &'a WorkspaceResolver<CliSys>,
  npm_system_info: NpmSystemInfo,
  is_desktop: bool,
}

impl<'a> DenoCompileBinaryWriter<'a> {
  #[allow(clippy::too_many_arguments, reason = "construction")]
  pub fn new(
    cjs_module_export_analyzer: &'a CliCjsModuleExportAnalyzer,
    cjs_tracker: &'a CliCjsTracker,
    cli_options: &'a CliOptions,
    deno_dir: &'a DenoDir,
    emitter: &'a CliEmitter,
    file_fetcher: &'a CliFileFetcher,
    http_client_provider: &'a HttpClientProvider,
    npm_resolver: &'a CliNpmResolver,
    workspace_resolver: &'a WorkspaceResolver<CliSys>,
    npm_system_info: NpmSystemInfo,
    is_desktop: bool,
  ) -> Self {
    Self {
      cjs_module_export_analyzer,
      cjs_tracker,
      cli_options,
      deno_dir,
      emitter,
      file_fetcher,
      http_client_provider,
      npm_resolver,
      workspace_resolver,
      npm_system_info,
      is_desktop,
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
      // Desktop builds handle icons during app bundle packaging.
      if !target.contains("windows") && !self.is_desktop {
        bail!(
          "The `--icon` flag is only available when targeting Windows (current: {})",
          target,
        );
      }
    }
    // Validate the resolved app name (explicit `--app-name` or the default
    // derived from the output file name) up front, so an invalid name fails
    // before we do any work to write the binary. The returned name is discarded
    // here; the value actually baked into the metadata is resolved again at the
    // write site below.
    resolve_app_name(
      options.compile_flags,
      options.display_output_filename,
      self.is_desktop,
    )?;
    self.write_standalone_binary(options, original_binary).await
  }

  async fn get_base_binary(
    &self,
    compile_flags: &CompileFlags,
  ) -> Result<Vec<u8>, AnyError> {
    if self.is_desktop {
      return self.get_desktop_base_binary(compile_flags).await;
    }

    // Used for testing.
    //
    // Phase 2 of the 'min sized' deno compile RFC talks
    // about adding this as a flag.
    if let Some(path) = get_dev_binary_path() {
      log::debug!("Resolved denort: {}", path.to_string_lossy());
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
    log::debug!("Resolved denort: {}", binary_path.display());

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
      exe_name: if target.contains("windows") {
        "denort.exe"
      } else {
        "denort"
      },
      archive_name: &binary_name,
      archive_data: &archive_data,
      dest_path: temp_dir.path(),
    })?;
    let base_binary = read_file(&base_binary_path)?;
    drop(temp_dir); // delete the temp dir
    Ok(base_binary)
  }

  async fn get_desktop_base_binary(
    &self,
    compile_flags: &CompileFlags,
  ) -> Result<Vec<u8>, AnyError> {
    // For development: check DENORT_DESKTOP_BIN env var or look
    // for libdenort next to the deno executable.
    if let Some(path) = get_dev_desktop_binary_path() {
      log::debug!("Resolved libdenort: {}", path.to_string_lossy());
      return std::fs::read(&path).with_context(|| {
        format!("Could not find libdenort at '{}'", path.to_string_lossy())
      });
    }

    let target = compile_flags.resolve_target();
    let lib_ext = if target.contains("darwin") {
      "dylib"
    } else if target.contains("windows") {
      "dll"
    } else {
      "so"
    };
    let lib_name = if target.contains("windows") {
      format!("denort.{lib_ext}")
    } else {
      format!("libdenort.{lib_ext}")
    };
    let binary_name = format!("libdenort-{target}.zip");

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
    log::debug!("Resolved libdenort: {}", binary_path.display());

    let read_file = |path: &Path| -> Result<Vec<u8>, AnyError> {
      std::fs::read(path).with_context(|| format!("Reading {}", path.display()))
    };
    let archive_data = if binary_path.exists() {
      read_file(&binary_path)?
    } else {
      self
        .download_base_binary(&binary_path, &binary_path_suffix)
        .await
        .context("Setting up desktop base binary.")?
    };
    let temp_dir = tempfile::TempDir::new()?;
    let base_binary_path = archive::unpack_into_dir(archive::UnpackArgs {
      exe_name: &lib_name,
      archive_name: &binary_name,
      archive_data: &archive_data,
      dest_path: temp_dir.path(),
    })?;
    let base_binary = read_file(&base_binary_path)?;
    drop(temp_dir);
    Ok(base_binary)
  }

  async fn download_base_binary(
    &self,
    output_path: &Path,
    binary_path_suffix: &str,
  ) -> Result<Vec<u8>, AnyError> {
    let download_url = format!("https://dl.deno.land/{binary_path_suffix}");
    let response = {
      let progress_bars = ProgressBar::new(ProgressBarStyle::DownloadBars);
      let progress = progress_bars.update(&download_url);

      self
        .http_client_provider
        .get_or_create()?
        .download_with_progress_and_retries(
          download_url.parse()?,
          &Default::default(),
          &progress,
        )
        .await?
    };
    let bytes = response
      .into_bytes()
      .with_context(|| format!("Failed downloading '{}'", download_url))?;

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
    // Embed the workspace package.json files so the standalone binary's node
    // resolver can read their "exports" (and other) fields at runtime. Without
    // this, resolving a workspace member by its package name falls back to
    // legacy `index.js` resolution instead of honoring the package's exports.
    for pkg_json in self.cli_options.workspace().package_jsons() {
      vfs.add_path(&pkg_json.path)?;
    }
    let progress_bar = ProgressBar::new(ProgressBarStyle::ProgressBars);
    // With --bundle the JS graph is self-contained, so the whole npm tree
    // is intentionally left out of the binary. The exception is packages
    // that ship native (.node) addons: the package JS is still bundled, but
    // its `.node` file imports stay external (`external = ["*.node"]` in
    // compile.rs) so the addon loader resolves them against the embedded VFS
    // at runtime. For that to work the package's installed folder, plus the
    // closure of its dependencies, must be embedded in the VFS.
    let npm_snapshot = if compile_flags.bundle {
      self
        .fill_bundle_native_addon_vfs(&mut vfs, &progress_bar)
        .context("Embedding native addon packages.")?
    } else {
      match &self.npm_resolver {
        CliNpmResolver::Managed(managed) => {
          if graph.modules().any(|m| m.npm().is_some()) {
            let snapshot = managed.resolution().snapshot();
            // When the user opts in (or via the existing unstable lazy-caching
            // path), prune the resolution snapshot to packages reachable from
            // npm specifiers in the graph. Otherwise embed the full snapshot
            // so non-statically-analyzable dynamic imports keep working.
            let snapshot = if compile_flags.exclude_unused_npm
              || self.cli_options.unstable_npm_lazy_caching()
            {
              let reqs = graph
                .specifiers()
                .filter_map(|(s, _)| {
                  NpmPackageReqReference::from_specifier(s)
                    .ok()
                    .map(|req_ref| req_ref.into_inner().req)
                })
                .collect::<Vec<_>>();
              snapshot.subset(&reqs)
            } else {
              snapshot
            }
            .as_valid_serialized_for_system(&self.npm_system_info);
            if !snapshot.as_serialized().packages.is_empty() {
              self
                .fill_npm_vfs(&mut vfs, Some(&snapshot), &progress_bar)
                .context("Building npm vfs.")?;
              Some(snapshot)
            } else {
              None
            }
          } else {
            None
          }
        }
        CliNpmResolver::Byonm(_) => {
          self.fill_npm_vfs(&mut vfs, None, &progress_bar)?;
          None
        }
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
    let mut asset_module_urls = graph.asset_module_urls();
    let progress =
      progress_bar.update_with_prompt(ProgressMessagePrompt::Compile, "");
    progress.set_total_size(specifiers_count as u64);
    let mut modules_done: u64 = 0;
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
          let original_bytes = match m.source.try_get_original_bytes() {
            Some(bytes) => bytes,
            None => self.load_asset_bypass_permissions(specifier).await?.source,
          };
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
                  Some(Cow::Borrowed(m.source.text.as_ref())),
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
              self.emitter.emit_source_for_deno_compile(
                &m.specifier,
                m.media_type,
                module_kind,
                &m.source.text,
              )?;
            if source != m.source.text.as_ref() {
              maybe_source_map = Some(source_map.into_bytes());
              maybe_transpiled = Some(source.into_bytes());
            }
          }
          (Some(original_bytes), m.media_type)
        }
        deno_graph::Module::Json(m) => {
          let original_bytes = match m.source.try_get_original_bytes() {
            Some(bytes) => bytes,
            None => {
              self
                .load_asset_bypass_permissions(&m.specifier)
                .await?
                .source
            }
          };
          (Some(original_bytes), m.media_type)
        }
        deno_graph::Module::Wasm(m) => {
          (Some(m.source.clone()), MediaType::Wasm)
        }
        deno_graph::Module::Npm(_)
        | deno_graph::Module::Node(_)
        | deno_graph::Module::External(_) => (None, MediaType::Unknown),
      };
      if let Some(original_source) = maybe_original_source {
        asset_module_urls.swap_remove(module.specifier());
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
              is_valid_utf8: is_valid_utf8(&original_source),
              data: Cow::Owned(original_source.to_vec()),
              maybe_transpiled: maybe_transpiled.map(Cow::Owned),
              maybe_source_map: maybe_source_map.map(Cow::Owned),
              maybe_cjs_export_analysis: maybe_cjs_export_analysis
                .map(Cow::Owned),
            },
          );
        }
      }
      modules_done += 1;
      progress.set_position(modules_done);
    }
    drop(progress);

    for url in asset_module_urls {
      if graph.try_get(url).is_err() {
        // skip because there was an error loading this module
        continue;
      }
      match url.scheme() {
        "file" => {
          let file_path = deno_path_util::url_to_file_path(url)?;
          vfs.add_path(&file_path)?;
        }
        "http" | "https" => {
          let specifier_id = specifier_store.get_or_add(url);
          if !remote_modules_store.contains(specifier_id) {
            // it's ok to bypass permissions here because we verified the module
            // loaded successfully in the graph
            let file = self.load_asset_bypass_permissions(url).await?;
            remote_modules_store.add(
              specifier_id,
              RemoteModuleEntry {
                media_type: MediaType::from_specifier_and_headers(
                  &file.url,
                  file.maybe_headers.as_ref(),
                ),
                is_valid_utf8: is_valid_utf8(&file.source),
                data: Cow::Owned(file.source.to_vec()),
                maybe_cjs_export_analysis: None,
                maybe_source_map: None,
                maybe_transpiled: None,
              },
            );
          }
        }
        _ => {}
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

    if let Some(import_map) = self.workspace_resolver.maybe_import_map()
      && let Ok(file_path) = url_to_file_path(import_map.base_url())
      && let Some(import_map_parent_dir) = file_path.parent()
    {
      // tell the vfs about the import map's parent directory in case it
      // falls outside what the root of where the VFS will be based
      vfs.add_possible_min_root_dir(import_map_parent_dir);
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
      // Only script-flavored files can carry CJS exports. Extensions answer
      // this for everything except extensionless files (`MediaType::Unknown`),
      // which may be real modules (an npm `"main"` with no extension — see
      // test-module-main-extension-lookup); those are disambiguated by content
      // below rather than skipped outright.
      if !matches!(
        media_type,
        MediaType::JavaScript
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::Jsx
          | MediaType::TypeScript
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Tsx
          | MediaType::Dts
          | MediaType::Dmts
          | MediaType::Dcts
          | MediaType::Unknown
      ) {
        continue;
      }
      if self.cjs_tracker.is_maybe_cjs(&specifier, media_type)? {
        // Strict UTF-8 (not `from_utf8_lossy`): binary assets (images,
        // fonts, …) that resolve to `Unknown` are skipped rather than
        // mangled into garbage that panics swc. Extensionless *text*
        // modules still flow through.
        let Some(bytes) = vfs.file_bytes(file.offset) else {
          continue;
        };
        let Ok(source) = std::str::from_utf8(bytes) else {
          continue;
        };
        let cjs_analysis_result = self
          .cjs_module_export_analyzer
          .analyze_all_exports(&specifier, Some(source.into()))
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

    let env_vars_from_env_file = {
      let mut aggregated_env_vars = IndexMap::new();
      for env_file_name in self.cli_options.env_file_names().rev() {
        match deno_dotenv::find_path_and_content(
          &CliSys::default(),
          self.cli_options.initial_cwd(),
          env_file_name,
        ) {
          Ok(Some((env_file_path, content))) => {
            match get_file_env_vars(&content) {
              Ok(env_vars) => {
                aggregated_env_vars.extend(env_vars);
                log::info!(
                  "{} Environment variables from the file \"{}\" were embedded in the generated executable file",
                  crate::colors::yellow("Warning"),
                  env_file_path.display()
                );
              }
              Err(e) => {
                handle_dotenv_error(
                  &e,
                  &env_file_path,
                  self.cli_options.log_level(),
                );
              }
            };
          }
          Ok(None) => {
            handle_dotenv_not_found(
              env_file_name,
              self.cli_options.log_level(),
            );
          }
          Err(e) => {
            handle_dotenv_io_error(&e, self.cli_options.log_level());
          }
        };
      }
      aggregated_env_vars
    };

    output_vfs(&vfs, display_output_filename);

    let preload_modules = self
      .cli_options
      .preload_modules()?
      .into_iter()
      .map(|s| root_dir_url.specifier_key(&s).into_owned())
      .collect::<Vec<_>>();

    let require_modules = self
      .cli_options
      .require_modules()?
      .into_iter()
      .map(|s| root_dir_url.specifier_key(&s).into_owned())
      .collect::<Vec<_>>();

    let metadata = Metadata {
      argv: compile_flags.args.clone(),
      seed: self.cli_options.seed(),
      code_cache_key,
      location: self.cli_options.location_flag().clone(),
      permissions: self.cli_options.permissions_options()?,
      v8_flags: construct_v8_flags(
        &get_default_v8_flags(),
        self.cli_options.v8_flags(),
        vec![],
      ),
      unsafely_ignore_certificate_errors: self
        .cli_options
        .unsafely_ignore_certificate_errors()
        .clone(),
      log_level: self.cli_options.log_level(),
      ca_stores: self.cli_options.ca_stores().clone(),
      ca_data,
      env_vars_from_env_file,
      entrypoint_key: root_dir_url.specifier_key(entrypoint).into_owned(),
      preload_modules,
      require_modules,
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
          .iter()
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
        catalogs: self.workspace_resolver.catalogs().clone(),
      },
      node_modules,
      unstable_config: UnstableConfig {
        legacy_flag_enabled: false,
        detect_cjs: self.cli_options.unstable_detect_cjs(),
        features: self
          .cli_options
          .unstable_features()
          .into_iter()
          .map(|s| s.to_string())
          .collect(),
        lazy_dynamic_imports: self.cli_options.unstable_lazy_dynamic_imports(),
        npm_lazy_caching: self.cli_options.unstable_npm_lazy_caching(),
        raw_imports: self.cli_options.unstable_raw_imports(),
        sloppy_imports: self.cli_options.unstable_sloppy_imports(),
      },
      otel_config: self.cli_options.otel_config(),
      vfs_case_sensitivity: vfs.case_sensitivity,
      self_extracting: if compile_flags.self_extracting {
        let mut hasher = FastInsecureHasher::new_deno_versioned();
        for file in &vfs.files {
          hasher.write_u64(file.len() as u64);
          hasher.write(file);
        }
        Some(format!("{:016x}", hasher.finish()))
      } else {
        None
      },
      // Bake in a stable app identity so origin-bound storage (default
      // `Deno.openKv()`, `localStorage`, `caches`) persists to a per-app
      // directory at runtime. Prefer an explicit `--app-name`, otherwise derive
      // it from the output file name (minus any executable extension Deno adds).
      // Resolving here keeps the identity stable even if the binary is later
      // renamed. The name is already validated in `write_bin` (via
      // `resolve_app_name`).
      app_name: Some(resolve_app_name(
        compile_flags,
        display_output_filename,
        self.is_desktop,
      )?),
      app_version: self
        .cli_options
        .workspace()
        .root_deno_json()
        .and_then(|c| c.json.version.clone()),
      error_reporting_url: self
        .cli_options
        .start_dir
        .to_desktop_config()
        .ok()
        .and_then(|c| c.error_reporting.as_ref()?.url.clone()),
      release_base_url: self
        .cli_options
        .start_dir
        .to_desktop_config()
        .ok()
        .and_then(|c| c.release.as_ref()?.base_url.clone()),
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

    let hs = |n: usize| crate::util::display::human_size(n as f64);
    let runtime_size = original_bin.len();
    let payload_size = data_section_bytes.len();
    let total_size = runtime_size + payload_size;

    // Payload breakdown: the data appended on top of the embedded runtime.
    // Zero-valued sections are omitted to avoid noise (e.g. a script with no
    // remote modules).
    log::info!("\n{} {}", crate::colors::bold("Payload"), hs(payload_size));
    if section_sizes.vfs > 0 {
      log::info!(
        "  {}  {}",
        crate::colors::gray("Source files  "),
        hs(section_sizes.vfs)
      );
    }
    if section_sizes.remote_modules > 0 {
      log::info!(
        "  {}  {}",
        crate::colors::gray("Remote modules"),
        hs(section_sizes.remote_modules)
      );
    }
    if section_sizes.metadata > 0 {
      log::info!(
        "  {}  {}",
        crate::colors::gray("Metadata      "),
        hs(section_sizes.metadata)
      );
    }

    // Headline: the actual on-disk size of the produced executable, split
    // into the runtime base and the user payload so the total is explained
    // rather than surprising. "\u{2192}" is an arrow, "\u{b7}" a middot.
    log::info!(
      "\n{} {} {} {}",
      crate::colors::green("Compiled"),
      display_output_filename,
      crate::colors::bold(format!("\u{2192} {}", hs(total_size))),
      crate::colors::gray(format!(
        "({} runtime + {} payload \u{b7} {})",
        hs(runtime_size),
        hs(payload_size),
        compile_flags.resolve_target(),
      )),
    );

    write_binary_bytes(writer, original_bin, data_section_bytes, compile_flags)
      .context("Writing binary bytes")
  }

  async fn load_asset_bypass_permissions(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<
    deno_cache_dir::file_fetcher::File,
    deno_resolver::file_fetcher::FetchError,
  > {
    self
      .file_fetcher
      .fetch_with_options(
        specifier,
        FetchPermissionsOptionRef::AllowAll,
        FetchOptions {
          local: FetchLocalOptions {
            include_mtime: false,
          },
          maybe_auth: None,
          maybe_accept: None,
          maybe_cache_setting: Some(
            &deno_cache_dir::file_fetcher::CacheSetting::Use,
          ),
        },
      )
      .await
  }

  /// Decide what to embed for `deno compile --bundle`. The bundle is
  /// always shipped; this controls the npm portion. We need it when
  /// either the CJS-from-ESM wrapper pointed at on-disk paths during
  /// rewriting, or the resolved tree has a native (`.node`) addon — in
  /// both cases the compiled binary will do node-module resolution at
  /// runtime. Pure-ESM bundles with no native addons skip this and ship
  /// nothing npm-related.
  ///
  /// When embedding is needed, we ship only the packages actually
  /// reached: the rewriter recorded every absolute path it pointed at,
  /// and we map each path back to its owning npm package and walk that
  /// closure. The full resolution snapshot still goes in the metadata
  /// so denort can resolve packages by name at runtime.
  fn fill_bundle_native_addon_vfs(
    &self,
    builder: &mut VfsBuilder,
    progress_bar: &ProgressBar,
  ) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, AnyError> {
    let needs_for_cjs_wrapper =
      self.cli_options.compile_bundle_embed_node_modules();
    let referenced_paths = self.cli_options.compile_bundle_referenced_paths();
    // For BYONM the addon scan walks the workspace `node_modules` trees, so
    // it needs the workspace root (managed npm ignores it).
    let workspace_root = self
      .cli_options
      .workspace()
      .root_dir_url()
      .to_file_path()
      .ok();
    let needs_for_native_addons = !needs_for_cjs_wrapper
      && !super::native_addons::find_native_addon_packages(
        self.npm_resolver,
        &self.npm_system_info,
        workspace_root.as_deref(),
      )?
      .is_empty();
    if !needs_for_cjs_wrapper && !needs_for_native_addons {
      return Ok(None);
    }

    match self.npm_resolver {
      CliNpmResolver::Managed(managed) => {
        let snapshot = managed
          .resolution()
          .snapshot()
          .as_valid_serialized_for_system(&self.npm_system_info);
        if snapshot.as_serialized().packages.is_empty() {
          return Ok(None);
        }
        // `collect_bundle_required_packages` only returns `None` for BYONM,
        // which is handled by the `CliNpmResolver::Byonm` arm below, so a
        // managed resolver always yields `Some` here.
        let Some(needed_ids) =
          super::native_addons::collect_bundle_required_packages(
            self.npm_resolver,
            &self.npm_system_info,
            referenced_paths,
          )?
        else {
          unreachable!(
            "collect_bundle_required_packages returns None only for BYONM"
          );
        };
        let progress =
          progress_bar.update_with_prompt(ProgressMessagePrompt::Compile, "");
        progress.set_total_size(needed_ids.len() as u64);
        // Dedup the set of `<deno-cache>/<id>/node_modules/` directories we
        // add: a single id's node_modules dir contains the canonical package
        // folder plus sibling symlinks to its direct deps. Going one level up
        // from the canonical folder picks both up so node-module resolution at
        // runtime can follow the symlink chain (e.g. the NAPI-RS
        // platform-specific sibling package).
        let mut embedded_roots: std::collections::HashSet<PathBuf> =
          std::collections::HashSet::new();
        let mut done: u64 = 0;
        for id in &needed_ids {
          if let Ok(folder) = managed.resolve_pkg_folder_from_pkg_id(id)
            && folder.exists()
          {
            let root_to_add =
              pkg_folder_node_modules_root(&folder).unwrap_or(folder.as_path());
            if embedded_roots.insert(root_to_add.to_path_buf()) {
              builder.add_dir_recursive(root_to_add).with_context(|| {
                format!("Embedding npm package at '{}'", root_to_add.display())
              })?;
            }
          }
          done += 1;
          progress.set_position(done);
        }
        Ok(Some(snapshot))
      }
      CliNpmResolver::Byonm(_) => {
        self.fill_npm_vfs(builder, None, progress_bar)?;
        Ok(None)
      }
    }
  }

  fn fill_npm_vfs(
    &self,
    builder: &mut VfsBuilder,
    snapshot: Option<&ValidSerializedNpmResolutionSnapshot>,
    progress_bar: &ProgressBar,
  ) -> Result<(), AnyError> {
    fn maybe_warn_different_system(system_info: &NpmSystemInfo) {
      if system_info != &NpmSystemInfo::default() {
        log::warn!(
          "{} The node_modules directory may be incompatible with the target system.",
          crate::colors::yellow("Warning")
        );
      }
    }

    match &self.npm_resolver {
      CliNpmResolver::Managed(npm_resolver) => {
        if let Some(node_modules_path) = npm_resolver.root_node_modules_path() {
          maybe_warn_different_system(&self.npm_system_info);
          let _progress =
            progress_bar.update_with_prompt(ProgressMessagePrompt::Compile, "");
          builder.add_dir_recursive(node_modules_path)?;
          Ok(())
        } else {
          let snapshot = snapshot.unwrap();
          // we'll flatten to remove any custom registries later
          let mut packages =
            snapshot.as_serialized().packages.iter().collect::<Vec<_>>();
          packages.sort_by(|a, b| a.id.cmp(&b.id)); // determinism
          let current_system = NpmSystemInfo::default();
          let progress =
            progress_bar.update_with_prompt(ProgressMessagePrompt::Compile, "");
          progress.set_total_size(packages.len() as u64);
          let mut packages_done: u64 = 0;
          for package in packages {
            let folder =
              npm_resolver.resolve_pkg_folder_from_pkg_id(&package.id)?;
            if !package.system.matches_system(&current_system)
              && !folder.exists()
            {
              log::warn!(
                "{} Ignoring 'npm:{}' because it was not present on the current system.",
                crate::colors::yellow("Warning"),
                package.id
              );
            } else {
              builder.add_dir_recursive(&folder)?;
            }
            packages_done += 1;
            progress.set_position(packages_done);
          }
          drop(progress);
          Ok(())
        }
      }
      CliNpmResolver::Byonm(_) => {
        maybe_warn_different_system(&self.npm_system_info);
        let _progress =
          progress_bar.update_with_prompt(ProgressMessagePrompt::Compile, "");
        // traverse and add all the node_modules directories in the workspace
        let mut pending_dirs = VecDeque::new();
        pending_dirs.push_back(
          self
            .cli_options
            .workspace()
            .root_dir_url()
            .to_file_path()
            .unwrap(),
        );
        while let Some(pending_dir) = pending_dirs.pop_front() {
          let Ok(entries) = fs::read_dir(&pending_dir) else {
            // Don't bother surfacing this error as it might be an error
            // like "access denied". In this case, just skip over it.
            continue;
          };
          let mut entries = entries.filter_map(|e| e.ok()).collect::<Vec<_>>();
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
        //
        // A registry url may include a sub-path (e.g.
        // `http://mirrors.example.com/npm/`), in which case the on-disk cache
        // layout is `<global_cache>/<host>/<sub>/<pkg>/...` rather than
        // `<global_cache>/<host>/<pkg>/...`. Walk to each known registry's
        // package root before flattening so packages always end up directly
        // under `localhost/`.
        let known_registries_dirnames: Vec<String> =
          npm_resolver.known_registries_dirnames().to_vec();
        let mut localhost_entries: IndexMap<String, VfsEntry> = IndexMap::new();
        let mut registry_top_segments: HashSet<String> = HashSet::new();
        for registry_dirname in &known_registries_dirnames {
          if let Some(first) = registry_dirname.split('/').next()
            && !first.is_empty()
          {
            registry_top_segments.insert(first.to_string());
          }
          let registry_path = global_cache_root_path.join(registry_dirname);
          let Some(registry_dir) = vfs.get_dir_mut(&registry_path) else {
            continue;
          };
          for entry in registry_dir.entries.take_inner() {
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

        let Some(root_dir) = vfs.get_dir_mut(global_cache_root_path) else {
          return vfs.build();
        };

        root_dir.name = DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME.to_string();
        let mut new_entries = Vec::with_capacity(root_dir.entries.len());
        for entry in root_dir.entries.take_inner() {
          match &entry {
            VfsEntry::Dir(dir) if registry_top_segments.contains(&dir.name) => {
              // The packages under this registry host dir have already been
              // flattened into `localhost_entries`. Drop the (now empty)
              // intermediate directory tree so it isn't embedded twice.
            }
            _ => {
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

#[allow(clippy::too_many_arguments, reason = "private code")]
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
#[allow(clippy::too_many_arguments, reason = "private code")]
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

fn get_libdenort_path(deno_exe: PathBuf) -> Option<OsString> {
  let mut libdenort = deno_exe;
  if cfg!(target_os = "macos") {
    libdenort.set_file_name("libdenort.dylib");
  } else if cfg!(windows) {
    libdenort.set_file_name("denort.dll");
  } else {
    libdenort.set_file_name("libdenort.so");
  }
  libdenort.exists().then(|| libdenort.into_os_string())
}

fn get_dev_desktop_binary_path() -> Option<OsString> {
  env::var_os("DENORT_DESKTOP_BIN").or_else(|| {
    env::current_exe().ok().and_then(|exec_path| {
      if exec_path
        .components()
        .any(|component| component == Component::Normal("target".as_ref()))
      {
        // Prefer release libdenort (optimized) over debug.
        let target_dir = exec_path.parent().and_then(|p| p.parent());
        target_dir
          .and_then(|d| {
            get_libdenort_path(d.join("release").join("libdenort.dylib"))
          })
          .or_else(|| get_libdenort_path(exec_path.clone()))
      } else {
        None
      }
    })
  })
}

/// This function returns the environment variables specified
/// in the passed environment file.
fn get_file_env_vars(
  content: &str,
) -> Result<IndexMap<String, String>, deno_dotenv::ParseError> {
  let mut file_env_vars = IndexMap::new();
  for item in deno_dotenv::from_content_sanitized_iter_with_substitution(
    &CliSys::default(),
    content,
  )? {
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

#[cfg(test)]
mod tests {
  use super::default_app_name;

  #[test]
  fn default_app_name_strips_only_deno_added_extensions() {
    assert_eq!(default_app_name("speedgraph.exe", false), "speedgraph");
    assert_eq!(default_app_name("speedgraph.so", false), "speedgraph.so");

    assert_eq!(default_app_name("speedgraph.dll", true), "speedgraph");
    assert_eq!(default_app_name("speedgraph.dylib", true), "speedgraph");
    assert_eq!(default_app_name("speedgraph.so", true), "speedgraph");
    assert_eq!(default_app_name("speed.graph.so", true), "speed.graph");
  }
}
