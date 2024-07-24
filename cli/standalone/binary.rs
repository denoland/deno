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
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use deno_ast::ModuleSpecifier;
use deno_config::workspace::PackageJsonDepResolution;
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
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_node::PackageJson;
use deno_semver::npm::NpmVersionReqParseError;
use deno_semver::package::PackageReq;
use deno_semver::VersionReqSpecifierParseError;
use eszip::EszipRelativeFileBaseUrl;
use indexmap::IndexMap;
use log::Level;
use serde::Deserialize;
use serde::Serialize;

use crate::args::CaData;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::args::PackageJsonInstallDepsProvider;
use crate::args::PermissionFlags;
use crate::args::UnstableConfig;
use crate::cache::DenoDir;
use crate::file_fetcher::FileFetcher;
use crate::http_util::HttpClientProvider;
use crate::npm::CliNpmResolver;
use crate::npm::InnerCliNpmResolverRef;
use crate::standalone::virtual_fs::VfsEntry;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

use super::virtual_fs::FileBackedVfs;
use super::virtual_fs::VfsBuilder;
use super::virtual_fs::VfsRoot;
use super::virtual_fs::VirtualDirectory;

const MAGIC_TRAILER: &[u8; 8] = b"d3n0l4nd";

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

#[derive(Deserialize, Serialize)]
pub struct SerializedWorkspaceResolver {
  pub import_map: Option<SerializedWorkspaceResolverImportMap>,
  pub package_jsons: BTreeMap<String, serde_json::Value>,
  pub pkg_json_resolution: PackageJsonDepResolution,
}

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
  pub env_vars_from_env_file: HashMap<String, String>,
  pub workspace_resolver: SerializedWorkspaceResolver,
  pub entrypoint_key: String,
  pub node_modules: Option<NodeModules>,
  pub disable_deprecated_api_warning: bool,
  pub unstable_config: UnstableConfig,
}

pub fn load_npm_vfs(root_dir_path: PathBuf) -> Result<FileBackedVfs, AnyError> {
  let data = libsui::find_section("d3n0l4nd").unwrap();

  // We do the first part sync so it can complete quickly
  let trailer: [u8; TRAILER_SIZE] = data[0..TRAILER_SIZE].try_into().unwrap();
  let trailer = match Trailer::parse(&trailer)? {
    None => panic!("Could not find trailer"),
    Some(trailer) => trailer,
  };
  let data = &data[TRAILER_SIZE..];

  let vfs_data =
    &data[trailer.npm_vfs_pos as usize..trailer.npm_files_pos as usize];
  let mut dir: VirtualDirectory = serde_json::from_slice(vfs_data)?;

  // align the name of the directory with the root dir
  dir.name = root_dir_path
    .file_name()
    .unwrap()
    .to_string_lossy()
    .to_string();

  let fs_root = VfsRoot {
    dir,
    root_path: root_dir_path,
    start_file_offset: trailer.npm_files_pos,
  };
  Ok(FileBackedVfs::new(data.to_vec(), fs_root))
}

fn write_binary_bytes(
  mut file_writer: File,
  original_bin: Vec<u8>,
  metadata: &Metadata,
  eszip: eszip::EszipV2,
  npm_vfs: Option<&VirtualDirectory>,
  npm_files: &Vec<Vec<u8>>,
  compile_flags: &CompileFlags,
) -> Result<(), AnyError> {
  let metadata = serde_json::to_string(metadata)?.as_bytes().to_vec();
  let npm_vfs = serde_json::to_string(&npm_vfs)?.as_bytes().to_vec();
  let eszip_archive = eszip.into_bytes();

  let mut writer = Vec::new();

  // write the trailer, which includes the positions
  // of the data blocks in the file
  writer.write_all(&{
    let metadata_pos = eszip_archive.len() as u64;
    let npm_vfs_pos = metadata_pos + (metadata.len() as u64);
    let npm_files_pos = npm_vfs_pos + (npm_vfs.len() as u64);
    Trailer {
      eszip_pos: 0,
      metadata_pos,
      npm_vfs_pos,
      npm_files_pos,
    }
    .as_bytes()
  })?;

  writer.write_all(&eszip_archive)?;
  writer.write_all(&metadata)?;
  writer.write_all(&npm_vfs)?;
  for file in npm_files {
    writer.write_all(file)?;
  }

  let target = compile_flags.resolve_target();
  if target.contains("linux") {
    libsui::Elf::new(&original_bin).append(&writer, &mut file_writer)?;
  } else if target.contains("windows") {
    libsui::PortableExecutable::from(&original_bin)?
      .write_resource("d3n0l4nd", writer)?
      .build(&mut file_writer)?;
  } else if target.contains("darwin") {
    libsui::Macho::from(original_bin)?
      .write_section("d3n0l4nd", writer)?
      .build(&mut file_writer)?;
  }
  Ok(())
}

pub fn is_standalone_binary(exe_path: &Path) -> bool {
  let Ok(data) = std::fs::read(exe_path) else {
    return false;
  };

  libsui::utils::is_elf(&data)
    | libsui::utils::is_pe(&data)
    | libsui::utils::is_macho(&data)
}

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a standalone
/// binary by skipping over the trailer width at the end of the file,
/// then checking for the magic trailer string `d3n0l4nd`. If found,
/// the bundle is executed. If not, this function exits with `Ok(None)`.
pub fn extract_standalone(
  cli_args: Cow<Vec<OsString>>,
) -> Result<
  Option<impl Future<Output = Result<(Metadata, eszip::EszipV2), AnyError>>>,
  AnyError,
> {
  let Some(data) = libsui::find_section("d3n0l4nd") else {
    return Ok(None);
  };

  // We do the first part sync so it can complete quickly
  let trailer: [u8; TRAILER_SIZE] = data[0..TRAILER_SIZE].try_into().unwrap();
  let trailer = match Trailer::parse(&trailer)? {
    None => return Ok(None),
    Some(trailer) => trailer,
  };

  let cli_args = cli_args.into_owned();
  // If we have an eszip, read it out
  Ok(Some(async move {
    let bufreader =
      deno_core::futures::io::BufReader::new(&data[TRAILER_SIZE..]);

    let (eszip, loader) = eszip::EszipV2::parse(bufreader)
      .await
      .context("Failed to parse eszip header")?;

    let bufreader = loader.await.context("Failed to parse eszip archive")?;

    let mut metadata = String::new();

    bufreader
      .take(trailer.metadata_len())
      .read_to_string(&mut metadata)
      .await
      .context("Failed to read metadata from the current executable")?;

    let mut metadata: Metadata = serde_json::from_str(&metadata).unwrap();
    metadata.argv.reserve(cli_args.len() - 1);
    for arg in cli_args.into_iter().skip(1) {
      metadata.argv.push(arg.into_string().unwrap());
    }

    Ok((metadata, eszip))
  }))
}

const TRAILER_SIZE: usize = std::mem::size_of::<Trailer>() + 8; // 8 bytes for the magic trailer string

struct Trailer {
  eszip_pos: u64,
  metadata_pos: u64,
  npm_vfs_pos: u64,
  npm_files_pos: u64,
}

impl Trailer {
  pub fn parse(trailer: &[u8]) -> Result<Option<Trailer>, AnyError> {
    let (magic_trailer, rest) = trailer.split_at(8);
    if magic_trailer != MAGIC_TRAILER {
      return Ok(None);
    }

    let (eszip_archive_pos, rest) = rest.split_at(8);
    let (metadata_pos, rest) = rest.split_at(8);
    let (npm_vfs_pos, npm_files_pos) = rest.split_at(8);
    let eszip_archive_pos = u64_from_bytes(eszip_archive_pos)?;
    let metadata_pos = u64_from_bytes(metadata_pos)?;
    let npm_vfs_pos = u64_from_bytes(npm_vfs_pos)?;
    let npm_files_pos = u64_from_bytes(npm_files_pos)?;
    Ok(Some(Trailer {
      eszip_pos: eszip_archive_pos,
      metadata_pos,
      npm_vfs_pos,
      npm_files_pos,
    }))
  }

  pub fn metadata_len(&self) -> u64 {
    self.npm_vfs_pos - self.metadata_pos
  }

  pub fn npm_vfs_len(&self) -> u64 {
    self.npm_files_pos - self.npm_vfs_pos
  }

  pub fn as_bytes(&self) -> Vec<u8> {
    let mut trailer = MAGIC_TRAILER.to_vec();
    trailer.write_all(&self.eszip_pos.to_be_bytes()).unwrap();
    trailer.write_all(&self.metadata_pos.to_be_bytes()).unwrap();
    trailer.write_all(&self.npm_vfs_pos.to_be_bytes()).unwrap();
    trailer
      .write_all(&self.npm_files_pos.to_be_bytes())
      .unwrap();
    trailer
  }
}

fn u64_from_bytes(arr: &[u8]) -> Result<u64, AnyError> {
  let fixed_arr: &[u8; 8] = arr
    .try_into()
    .context("Failed to convert the buffer into a fixed-size array")?;
  Ok(u64::from_be_bytes(*fixed_arr))
}

pub fn unpack_into_dir(
  exe_name: &str,
  archive_name: &str,
  archive_data: Vec<u8>,
  is_windows: bool,
  temp_dir: &tempfile::TempDir,
) -> Result<PathBuf, AnyError> {
  let temp_dir_path = temp_dir.path();
  let exe_ext = if is_windows { "exe" } else { "" };
  let archive_path = temp_dir_path.join(exe_name).with_extension("zip");
  let exe_path = temp_dir_path.join(exe_name).with_extension(exe_ext);
  assert!(!exe_path.exists());

  let archive_ext = Path::new(archive_name)
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap();
  let unpack_status = match archive_ext {
    "zip" if cfg!(windows) => {
      fs::write(&archive_path, &archive_data)?;
      Command::new("tar.exe")
        .arg("xf")
        .arg(&archive_path)
        .arg("-C")
        .arg(temp_dir_path)
        .spawn()
        .map_err(|err| {
          if err.kind() == std::io::ErrorKind::NotFound {
            std::io::Error::new(
              std::io::ErrorKind::NotFound,
              "`tar.exe` was not found in your PATH",
            )
          } else {
            err
          }
        })?
        .wait()?
    }
    "zip" => {
      fs::write(&archive_path, &archive_data)?;
      Command::new("unzip")
        .current_dir(temp_dir_path)
        .arg(&archive_path)
        .spawn()
        .map_err(|err| {
          if err.kind() == std::io::ErrorKind::NotFound {
            std::io::Error::new(
              std::io::ErrorKind::NotFound,
              "`unzip` was not found in your PATH, please install `unzip`",
            )
          } else {
            err
          }
        })?
        .wait()?
    }
    ext => bail!("Unsupported archive type: '{ext}'"),
  };
  if !unpack_status.success() {
    bail!("Failed to unpack archive.");
  }
  assert!(exe_path.exists());
  fs::remove_file(&archive_path)?;
  Ok(exe_path)
}

pub struct DenoCompileBinaryWriter<'a> {
  deno_dir: &'a DenoDir,
  file_fetcher: &'a FileFetcher,
  http_client_provider: &'a HttpClientProvider,
  npm_resolver: &'a dyn CliNpmResolver,
  workspace_resolver: &'a WorkspaceResolver,
  npm_system_info: NpmSystemInfo,
}

impl<'a> DenoCompileBinaryWriter<'a> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    deno_dir: &'a DenoDir,
    file_fetcher: &'a FileFetcher,
    http_client_provider: &'a HttpClientProvider,
    npm_resolver: &'a dyn CliNpmResolver,
    workspace_resolver: &'a WorkspaceResolver,
    npm_system_info: NpmSystemInfo,
  ) -> Self {
    Self {
      deno_dir,
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
    eszip: eszip::EszipV2,
    root_dir_url: EszipRelativeFileBaseUrl<'_>,
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
    self.write_standalone_binary(
      writer,
      original_binary,
      eszip,
      root_dir_url,
      entrypoint,
      cli_options,
      compile_flags,
    )
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

    let binary_path_suffix = if crate::version::is_canary() {
      format!("canary/{}/{}", crate::version::GIT_COMMIT_HASH, binary_name)
    } else {
      format!("release/v{}/{}", env!("CARGO_PKG_VERSION"), binary_name)
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
    let base_binary_path = unpack_into_dir(
      "denort",
      &binary_name,
      archive_data,
      target.contains("windows"),
      &temp_dir,
    )?;
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
        .download_with_progress(download_url.parse()?, None, &progress)
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
  fn write_standalone_binary(
    &self,
    writer: File,
    original_bin: Vec<u8>,
    mut eszip: eszip::EszipV2,
    root_dir_url: EszipRelativeFileBaseUrl<'_>,
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
    let (npm_vfs, npm_files, node_modules) = match self.npm_resolver.as_inner()
    {
      InnerCliNpmResolverRef::Managed(managed) => {
        let snapshot =
          managed.serialized_valid_snapshot_for_system(&self.npm_system_info);
        if !snapshot.as_serialized().packages.is_empty() {
          let (root_dir, files) = self
            .build_vfs(&root_path, cli_options)?
            .into_dir_and_files();
          eszip.add_npm_snapshot(snapshot);
          (
            Some(root_dir),
            files,
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
          )
        } else {
          (None, Vec::new(), None)
        }
      }
      InnerCliNpmResolverRef::Byonm(resolver) => {
        let (root_dir, files) = self
          .build_vfs(&root_path, cli_options)?
          .into_dir_and_files();
        (
          Some(root_dir),
          files,
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
        )
      }
    };

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
      disable_deprecated_api_warning: cli_options
        .disable_deprecated_api_warning,
      unstable_config: UnstableConfig {
        legacy_flag_enabled: cli_options.legacy_unstable_flag(),
        bare_node_builtins: cli_options.unstable_bare_node_builtins(),
        byonm: cli_options.use_byonm(),
        sloppy_imports: cli_options.unstable_sloppy_imports(),
        features: cli_options.unstable_features(),
      },
    };

    write_binary_bytes(
      writer,
      original_bin,
      &metadata,
      eszip,
      npm_vfs.as_ref(),
      &npm_files,
      compile_flags,
    )
  }

  fn build_vfs(
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
          let root_path = npm_resolver.global_cache_root_folder();
          let mut builder = VfsBuilder::new(root_path)?;
          for package in npm_resolver.all_system_packages(&self.npm_system_info)
          {
            let folder =
              npm_resolver.resolve_pkg_folder_from_pkg_id(&package.id)?;
            builder.add_dir_recursive(&folder)?;
          }

          // Flatten all the registries folders into a single "node_modules/localhost" folder
          // that will be used by denort when loading the npm cache. This avoids us exposing
          // the user's private registry information and means we don't have to bother
          // serializing all the different registry config into the binary.
          builder.with_root_dir(|root_dir| {
            root_dir.name = "node_modules".to_string();
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
          let entries = fs::read_dir(&pending_dir).with_context(|| {
            format!("Failed reading: {}", pending_dir.display())
          })?;
          for entry in entries {
            let entry = entry?;
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
) -> Result<HashMap<String, String>, dotenvy::Error> {
  let mut file_env_vars = HashMap::new();
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
