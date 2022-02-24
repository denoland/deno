// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::deno_dir::DenoDir;
use crate::flags::CheckFlag;
use crate::flags::DenoSubcommand;
use crate::flags::Flags;
use crate::flags::RunFlags;
use crate::standalone::Metadata;
use crate::standalone::MAGIC_TRAILER;
use crate::ProcState;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::ModuleSpecifier;
use deno_runtime::deno_fetch::reqwest::Client;
use deno_runtime::permissions::Permissions;
use std::env;
use std::fs::read;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

pub async fn get_base_binary(
  deno_dir: &DenoDir,
  target: Option<String>,
) -> Result<Vec<u8>, AnyError> {
  if target.is_none() {
    let path = std::env::current_exe()?;
    return Ok(tokio::fs::read(path).await?);
  }

  let target = target.unwrap_or_else(|| env!("TARGET").to_string());
  let binary_name = format!("deno-{}.zip", target);

  let binary_path_suffix = if crate::version::is_canary() {
    format!("canary/{}/{}", crate::version::GIT_COMMIT_HASH, binary_name)
  } else {
    format!("release/v{}/{}", env!("CARGO_PKG_VERSION"), binary_name)
  };

  let download_directory = deno_dir.root.join("dl");
  let binary_path = download_directory.join(&binary_path_suffix);

  if !binary_path.exists() {
    download_base_binary(&download_directory, &binary_path_suffix).await?;
  }

  let archive_data = tokio::fs::read(binary_path).await?;
  let base_binary_path =
    crate::tools::upgrade::unpack(archive_data, target.contains("windows"))?;
  let base_binary = tokio::fs::read(base_binary_path).await?;
  Ok(base_binary)
}

async fn download_base_binary(
  output_directory: &Path,
  binary_path_suffix: &str,
) -> Result<(), AnyError> {
  let download_url = format!("https://dl.deno.land/{}", binary_path_suffix);

  let client_builder = Client::builder();
  let client = client_builder.build()?;

  println!("Checking {}", &download_url);

  let res = client.get(&download_url).send().await?;

  let binary_content = if res.status().is_success() {
    println!("Download has been found");
    res.bytes().await?.to_vec()
  } else {
    println!("Download could not be found, aborting");
    std::process::exit(1)
  };

  std::fs::create_dir_all(&output_directory)?;
  let output_path = output_directory.join(binary_path_suffix);
  std::fs::create_dir_all(&output_path.parent().unwrap())?;
  tokio::fs::write(output_path, binary_content).await?;
  Ok(())
}

/// This functions creates a standalone deno binary by appending a bundle
/// and magic trailer to the currently executing binary.
pub async fn create_standalone_binary(
  mut original_bin: Vec<u8>,
  eszip: eszip::EszipV2,
  entrypoint: ModuleSpecifier,
  flags: Flags,
  ps: ProcState,
) -> Result<Vec<u8>, AnyError> {
  let mut eszip_archive = eszip.into_bytes();

  let ca_data = match &flags.ca_file {
    Some(ca_file) => Some(read(ca_file)?),
    None => None,
  };
  let maybe_import_map: Option<(Url, String)> = match flags
    .import_map_path
    .as_ref()
  {
    None => None,
    Some(import_map_url) => {
      let import_map_specifier = deno_core::resolve_url_or_path(import_map_url)
        .context(format!("Bad URL (\"{}\") for import map.", import_map_url))?;
      let file = ps
        .file_fetcher
        .fetch(&import_map_specifier, &mut Permissions::allow_all())
        .await
        .context(format!(
          "Unable to load '{}' import map",
          import_map_specifier
        ))?;

      Some((import_map_specifier, file.source.to_string()))
    }
  };
  let metadata = Metadata {
    argv: flags.argv.clone(),
    unstable: flags.unstable,
    seed: flags.seed,
    location: flags.location.clone(),
    permissions: flags.permissions_options(),
    v8_flags: flags.v8_flags.clone(),
    unsafely_ignore_certificate_errors: flags
      .unsafely_ignore_certificate_errors
      .clone(),
    log_level: flags.log_level,
    ca_stores: flags.ca_stores,
    ca_data,
    entrypoint,
    maybe_import_map,
  };
  let mut metadata = serde_json::to_string(&metadata)?.as_bytes().to_vec();

  let eszip_pos = original_bin.len();
  let metadata_pos = eszip_pos + eszip_archive.len();
  let mut trailer = MAGIC_TRAILER.to_vec();
  trailer.write_all(&eszip_pos.to_be_bytes())?;
  trailer.write_all(&metadata_pos.to_be_bytes())?;

  let mut final_bin = Vec::with_capacity(
    original_bin.len() + eszip_archive.len() + trailer.len(),
  );
  final_bin.append(&mut original_bin);
  final_bin.append(&mut eszip_archive);
  final_bin.append(&mut metadata);
  final_bin.append(&mut trailer);

  Ok(final_bin)
}

/// This function writes out a final binary to specified path. If output path
/// is not already standalone binary it will return error instead.
pub async fn write_standalone_binary(
  output: PathBuf,
  target: Option<String>,
  final_bin: Vec<u8>,
) -> Result<(), AnyError> {
  let output = match target {
    Some(target) => {
      if target.contains("windows") {
        output.with_extension("exe")
      } else {
        output
      }
    }
    None => {
      if cfg!(windows) && output.extension().unwrap_or_default() != "exe" {
        output.with_extension("exe")
      } else {
        output
      }
    }
  };

  if output.exists() {
    // If the output is a directory, throw error
    if output.is_dir() {
      bail!("Could not compile: {:?} is a directory.", &output);
    }

    // Make sure we don't overwrite any file not created by Deno compiler.
    // Check for magic trailer in last 24 bytes.
    let mut has_trailer = false;
    let mut output_file = File::open(&output)?;
    // This seek may fail because the file is too small to possibly be
    // `deno compile` output.
    if output_file.seek(SeekFrom::End(-24)).is_ok() {
      let mut trailer = [0; 24];
      output_file.read_exact(&mut trailer)?;
      let (magic_trailer, _) = trailer.split_at(8);
      has_trailer = magic_trailer == MAGIC_TRAILER;
    }
    if !has_trailer {
      bail!("Could not compile: cannot overwrite {:?}.", &output);
    }

    // Remove file if it was indeed a deno compiled binary, to avoid corruption
    // (see https://github.com/denoland/deno/issues/10310)
    std::fs::remove_file(&output)?;
  } else {
    let output_base = &output.parent().unwrap();
    if output_base.exists() && output_base.is_file() {
      bail!("Could not compile: {:?} is a file.", &output_base);
    }
    tokio::fs::create_dir_all(output_base).await?;
  }

  tokio::fs::write(&output, final_bin).await?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o777);
    tokio::fs::set_permissions(output, perms).await?;
  }

  Ok(())
}

/// Transform the flags passed to `deno compile` to flags that would be used at
/// runtime, as if `deno run` were used.
/// - Flags that affect module resolution, loading, type checking, etc. aren't
///   applicable at runtime so are set to their defaults like `false`.
/// - Other flags are inherited.
pub fn compile_to_runtime_flags(
  flags: &Flags,
  baked_args: Vec<String>,
) -> Result<Flags, AnyError> {
  // IMPORTANT: Don't abbreviate any of this to `..flags` or
  // `..Default::default()`. That forces us to explicitly consider how any
  // change to `Flags` should be reflected here.
  Ok(Flags {
    argv: baked_args,
    subcommand: DenoSubcommand::Run(RunFlags {
      script: "placeholder".to_string(),
    }),
    allow_all: flags.allow_all,
    allow_env: flags.allow_env.clone(),
    allow_hrtime: flags.allow_hrtime,
    allow_net: flags.allow_net.clone(),
    allow_ffi: flags.allow_ffi.clone(),
    allow_read: flags.allow_read.clone(),
    allow_run: flags.allow_run.clone(),
    allow_write: flags.allow_write.clone(),
    ca_stores: flags.ca_stores.clone(),
    ca_file: flags.ca_file.clone(),
    cache_blocklist: vec![],
    cache_path: None,
    cached_only: false,
    config_path: None,
    coverage_dir: flags.coverage_dir.clone(),
    enable_testing_features: false,
    ignore: vec![],
    import_map_path: flags.import_map_path.clone(),
    inspect_brk: None,
    inspect: None,
    location: flags.location.clone(),
    lock_write: false,
    lock: None,
    log_level: flags.log_level,
    check: CheckFlag::All,
    compat: flags.compat,
    unsafely_ignore_certificate_errors: flags
      .unsafely_ignore_certificate_errors
      .clone(),
    no_remote: false,
    no_prompt: flags.no_prompt,
    reload: false,
    repl: false,
    seed: flags.seed,
    unstable: flags.unstable,
    v8_flags: flags.v8_flags.clone(),
    version: false,
    watch: None,
    no_clear_screen: false,
  })
}
