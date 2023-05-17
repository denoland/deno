// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::env::current_exe;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::io::AllowStdIo;
use deno_core::futures::AsyncReadExt;
use deno_core::futures::AsyncSeekExt;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_npm::registry::PackageDepNpmSchemeValueParseError;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::NpmSystemInfo;
use deno_runtime::permissions::PermissionsOptions;
use deno_semver::npm::NpmPackageReq;
use deno_semver::npm::NpmVersionReqSpecifierParseError;
use log::Level;
use serde::Deserialize;
use serde::Serialize;

use crate::args::package_json::PackageJsonDepValueParseError;
use crate::args::package_json::PackageJsonDeps;
use crate::args::CaData;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::args::PackageJsonDepsProvider;
use crate::cache::DenoDir;
use crate::file_fetcher::FileFetcher;
use crate::http_util::HttpClient;
use crate::npm::CliNpmRegistryApi;
use crate::npm::CliNpmResolver;
use crate::npm::NpmCache;
use crate::npm::NpmResolution;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

use super::virtual_fs::FileBackedVfs;
use super::virtual_fs::VfsBuilder;
use super::virtual_fs::VfsRoot;
use super::virtual_fs::VirtualDirectory;

const MAGIC_TRAILER: &[u8; 8] = b"d3n0l4nd";

#[derive(Serialize, Deserialize)]
enum SerializablePackageJsonDepValueParseError {
  SchemeValue(String),
  Specifier(String),
  Unsupported { scheme: String },
}

impl SerializablePackageJsonDepValueParseError {
  pub fn from_err(err: PackageJsonDepValueParseError) -> Self {
    match err {
      PackageJsonDepValueParseError::SchemeValue(err) => {
        Self::SchemeValue(err.value)
      }
      PackageJsonDepValueParseError::Specifier(err) => {
        Self::Specifier(err.source.to_string())
      }
      PackageJsonDepValueParseError::Unsupported { scheme } => {
        Self::Unsupported { scheme }
      }
    }
  }

  pub fn into_err(self) -> PackageJsonDepValueParseError {
    match self {
      SerializablePackageJsonDepValueParseError::SchemeValue(value) => {
        PackageJsonDepValueParseError::SchemeValue(
          PackageDepNpmSchemeValueParseError { value },
        )
      }
      SerializablePackageJsonDepValueParseError::Specifier(source) => {
        PackageJsonDepValueParseError::Specifier(
          NpmVersionReqSpecifierParseError {
            source: monch::ParseErrorFailureError::new(source),
          },
        )
      }
      SerializablePackageJsonDepValueParseError::Unsupported { scheme } => {
        PackageJsonDepValueParseError::Unsupported { scheme }
      }
    }
  }
}

#[derive(Serialize, Deserialize)]
pub struct SerializablePackageJsonDeps(
  BTreeMap<
    String,
    Result<NpmPackageReq, SerializablePackageJsonDepValueParseError>,
  >,
);

impl SerializablePackageJsonDeps {
  pub fn from_deps(deps: PackageJsonDeps) -> Self {
    Self(
      deps
        .into_iter()
        .map(|(name, req)| {
          let res =
            req.map_err(SerializablePackageJsonDepValueParseError::from_err);
          (name, res)
        })
        .collect(),
    )
  }

  pub fn into_deps(self) -> PackageJsonDeps {
    self
      .0
      .into_iter()
      .map(|(name, res)| (name, res.map_err(|err| err.into_err())))
      .collect()
  }
}

#[derive(Deserialize, Serialize)]
pub struct Metadata {
  pub argv: Vec<String>,
  pub unstable: bool,
  pub seed: Option<u64>,
  pub permissions: PermissionsOptions,
  pub location: Option<Url>,
  pub v8_flags: Vec<String>,
  pub log_level: Option<Level>,
  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<Vec<u8>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub maybe_import_map: Option<(Url, String)>,
  pub entrypoint: ModuleSpecifier,
  /// Whether this uses a node_modules directory (true) or the global cache (false).
  pub node_modules_dir: bool,
  pub npm_snapshot: Option<SerializedNpmResolutionSnapshot>,
  pub package_json_deps: Option<SerializablePackageJsonDeps>,
}

pub fn load_npm_vfs(root_dir_path: PathBuf) -> Result<FileBackedVfs, AnyError> {
  let file_path = current_exe().unwrap();
  let mut file = std::fs::File::open(file_path)?;
  file.seek(SeekFrom::End(-(TRAILER_SIZE as i64)))?;
  let mut trailer = [0; TRAILER_SIZE];
  file.read_exact(&mut trailer)?;
  let trailer = Trailer::parse(&trailer)?.unwrap();
  file.seek(SeekFrom::Start(trailer.npm_vfs_pos))?;
  let mut vfs_data = vec![0; trailer.npm_vfs_len() as usize];
  file.read_exact(&mut vfs_data)?;
  let mut dir: VirtualDirectory = serde_json::from_slice(&vfs_data)?;

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
  Ok(FileBackedVfs::new(file, fs_root))
}

fn write_binary_bytes(
  writer: &mut impl Write,
  original_bin: Vec<u8>,
  metadata: &Metadata,
  eszip: eszip::EszipV2,
  npm_vfs: Option<&VirtualDirectory>,
  npm_files: &Vec<Vec<u8>>,
) -> Result<(), AnyError> {
  let metadata = serde_json::to_string(metadata)?.as_bytes().to_vec();
  let npm_vfs = serde_json::to_string(&npm_vfs)?.as_bytes().to_vec();
  let eszip_archive = eszip.into_bytes();

  writer.write_all(&original_bin)?;
  writer.write_all(&eszip_archive)?;
  writer.write_all(&metadata)?;
  writer.write_all(&npm_vfs)?;
  for file in npm_files {
    writer.write_all(file)?;
  }

  // write the trailer, which includes the positions
  // of the data blocks in the file
  writer.write_all(&{
    let eszip_pos = original_bin.len() as u64;
    let metadata_pos = eszip_pos + (eszip_archive.len() as u64);
    let npm_vfs_pos = metadata_pos + (metadata.len() as u64);
    let npm_files_pos = npm_vfs_pos + (npm_vfs.len() as u64);
    Trailer {
      eszip_pos,
      metadata_pos,
      npm_vfs_pos,
      npm_files_pos,
    }
    .as_bytes()
  })?;

  Ok(())
}

pub fn is_standalone_binary(exe_path: &Path) -> bool {
  let Ok(mut output_file) = std::fs::File::open(exe_path) else {
    return false;
  };
  if output_file
    .seek(SeekFrom::End(-(TRAILER_SIZE as i64)))
    .is_err()
  {
    // This seek may fail because the file is too small to possibly be
    // `deno compile` output.
    return false;
  }
  let mut trailer = [0; TRAILER_SIZE];
  if output_file.read_exact(&mut trailer).is_err() {
    return false;
  };
  let (magic_trailer, _) = trailer.split_at(8);
  magic_trailer == MAGIC_TRAILER
}

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a standalone
/// binary by skipping over the trailer width at the end of the file,
/// then checking for the magic trailer string `d3n0l4nd`. If found,
/// the bundle is executed. If not, this function exits with `Ok(None)`.
pub async fn extract_standalone(
  exe_path: &Path,
  cli_args: Vec<String>,
) -> Result<Option<(Metadata, eszip::EszipV2)>, AnyError> {
  let file = std::fs::File::open(exe_path)?;

  let mut bufreader =
    deno_core::futures::io::BufReader::new(AllowStdIo::new(file));

  let _trailer_pos = bufreader
    .seek(SeekFrom::End(-(TRAILER_SIZE as i64)))
    .await?;
  let mut trailer = [0; TRAILER_SIZE];
  bufreader.read_exact(&mut trailer).await?;
  let trailer = match Trailer::parse(&trailer)? {
    None => return Ok(None),
    Some(trailer) => trailer,
  };

  bufreader.seek(SeekFrom::Start(trailer.eszip_pos)).await?;

  let (eszip, loader) = eszip::EszipV2::parse(bufreader)
    .await
    .context("Failed to parse eszip header")?;

  let mut bufreader = loader.await.context("Failed to parse eszip archive")?;

  bufreader
    .seek(SeekFrom::Start(trailer.metadata_pos))
    .await?;

  let mut metadata = String::new();

  bufreader
    .take(trailer.metadata_len())
    .read_to_string(&mut metadata)
    .await
    .context("Failed to read metadata from the current executable")?;

  let mut metadata: Metadata = serde_json::from_str(&metadata).unwrap();
  metadata.argv.append(&mut cli_args[1..].to_vec());

  Ok(Some((metadata, eszip)))
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

pub struct DenoCompileBinaryWriter<'a> {
  file_fetcher: &'a FileFetcher,
  client: &'a HttpClient,
  deno_dir: &'a DenoDir,
  npm_api: &'a CliNpmRegistryApi,
  npm_cache: &'a NpmCache,
  npm_resolution: &'a NpmResolution,
  npm_resolver: &'a CliNpmResolver,
  npm_system_info: NpmSystemInfo,
  package_json_deps_provider: &'a PackageJsonDepsProvider,
}

impl<'a> DenoCompileBinaryWriter<'a> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    file_fetcher: &'a FileFetcher,
    client: &'a HttpClient,
    deno_dir: &'a DenoDir,
    npm_api: &'a CliNpmRegistryApi,
    npm_cache: &'a NpmCache,
    npm_resolution: &'a NpmResolution,
    npm_resolver: &'a CliNpmResolver,
    npm_system_info: NpmSystemInfo,
    package_json_deps_provider: &'a PackageJsonDepsProvider,
  ) -> Self {
    Self {
      file_fetcher,
      client,
      deno_dir,
      npm_api,
      npm_cache,
      npm_resolver,
      npm_system_info,
      npm_resolution,
      package_json_deps_provider,
    }
  }

  pub async fn write_bin(
    &self,
    writer: &mut impl Write,
    eszip: eszip::EszipV2,
    module_specifier: &ModuleSpecifier,
    compile_flags: &CompileFlags,
    cli_options: &CliOptions,
  ) -> Result<(), AnyError> {
    // Select base binary based on target
    let original_binary =
      self.get_base_binary(compile_flags.target.clone()).await?;

    self
      .write_standalone_binary(
        writer,
        original_binary,
        eszip,
        module_specifier,
        cli_options,
        compile_flags,
      )
      .await
  }

  async fn get_base_binary(
    &self,
    target: Option<String>,
  ) -> Result<Vec<u8>, AnyError> {
    if target.is_none() {
      let path = std::env::current_exe()?;
      return Ok(std::fs::read(path)?);
    }

    let target = target.unwrap_or_else(|| env!("TARGET").to_string());
    let binary_name = format!("deno-{target}.zip");

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
    let base_binary_path = crate::tools::upgrade::unpack_into_dir(
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
        .client
        .download_with_progress(download_url, &progress)
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
  async fn write_standalone_binary(
    &self,
    writer: &mut impl Write,
    original_bin: Vec<u8>,
    eszip: eszip::EszipV2,
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
    let maybe_import_map = cli_options
      .resolve_import_map(self.file_fetcher)
      .await?
      .map(|import_map| (import_map.base_url().clone(), import_map.to_json()));
    let (npm_snapshot, npm_vfs, npm_files) =
      if self.npm_resolution.has_packages() {
        let (root_dir, files) = self.build_vfs()?.into_dir_and_files();
        let snapshot = self.npm_resolution.serialized_snapshot();
        (Some(snapshot), Some(root_dir), files)
      } else {
        (None, None, Vec::new())
      };

    let metadata = Metadata {
      argv: compile_flags.args.clone(),
      unstable: cli_options.unstable(),
      seed: cli_options.seed(),
      location: cli_options.location_flag().clone(),
      permissions: cli_options.permissions_options(),
      v8_flags: cli_options.v8_flags().clone(),
      unsafely_ignore_certificate_errors: cli_options
        .unsafely_ignore_certificate_errors()
        .clone(),
      log_level: cli_options.log_level(),
      ca_stores: cli_options.ca_stores().clone(),
      ca_data,
      entrypoint: entrypoint.clone(),
      maybe_import_map,
      node_modules_dir: self.npm_resolver.node_modules_path().is_some(),
      npm_snapshot,
      package_json_deps: self
        .package_json_deps_provider
        .deps()
        .map(|deps| SerializablePackageJsonDeps::from_deps(deps.clone())),
    };

    write_binary_bytes(
      writer,
      original_bin,
      &metadata,
      eszip,
      npm_vfs.as_ref(),
      &npm_files,
    )
  }

  fn build_vfs(&self) -> Result<VfsBuilder, AnyError> {
    if let Some(node_modules_path) = self.npm_resolver.node_modules_path() {
      let mut builder = VfsBuilder::new(node_modules_path.clone());
      builder.add_dir_recursive(&node_modules_path)?;
      Ok(builder)
    } else {
      // DO NOT include the user's registry url as it may contain credentials,
      // but also don't make this dependent on the registry url
      let registry_url = self.npm_api.base_url();
      let root_path = self.npm_cache.registry_folder(registry_url);
      let mut builder = VfsBuilder::new(root_path);
      for package in self
        .npm_resolution
        .all_system_packages(&self.npm_system_info)
      {
        let folder = self
          .npm_resolver
          .resolve_pkg_folder_from_pkg_id(&package.pkg_id)?;
        builder.add_dir_recursive(&folder)?;
      }
      // overwrite the root directory's name to obscure the user's registry url
      builder.set_root_dir_name("node_modules".to_string());
      Ok(builder)
    }
  }
}
