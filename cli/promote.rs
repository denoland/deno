use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::value_parser;
use clap::Arg;
use clap::ArgAction;
use clap::ArgMatches;
use clap::Command;
use memmap2::MmapOptions;
use memmem::Searcher;
use memmem::TwoWaySearcher;
use version::DENO_METADATA_BLOCK_SIGNATURE;

use crate::version::ReleaseInfo;
use crate::version::ReleaseState;

mod version;

fn clap_root() -> Command {
  Command::new("promote")
    .subcommand(
      Command::new("patch")
        .arg(
          Arg::new("binary")
            .required(true)
            .value_parser(value_parser!(PathBuf)),
        )
        .arg(Arg::new("version").required(true))
        .arg(
          Arg::new("dry-run")
            .long("dry-run")
            .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("force").long("force").action(ArgAction::SetTrue)),
    )
    .subcommand(
      Command::new("read").arg(
        Arg::new("binary")
          .required(true)
          .value_parser(value_parser!(PathBuf)),
      ),
    )
}

fn pretty_print_release_info(info: &ReleaseInfo) {
  println!(" - Metadata header: {:x}", info.metadata_magic);
  println!(" - Metadata version: {}", info.metadata_version);
  println!(" - Git hash: {}", info.git_hash());
  println!(" - Version: {}", info.version_string());
  println!(" - State: {:?}", info.release_state);
}

fn read(
  binary: &Path,
  print: bool,
) -> Result<String, Box<dyn std::error::Error>> {
  let file = OpenOptions::new().read(true).open(binary)?;
  let mmap = unsafe { MmapOptions::new().map(&file)? };
  println!("Loaded {} bytes", mmap.len());
  let header = DENO_METADATA_BLOCK_SIGNATURE.to_le_bytes();
  let searcher = TwoWaySearcher::new(&header);
  let offset = searcher
    .search_in(&mmap)
    .ok_or("Failed to locate signature")?;
  println!("Found at offset {offset}");
  let ptr = unsafe { mmap.as_ptr().add(offset) as *mut _ };
  let release_info: ReleaseInfo = unsafe { std::ptr::read_volatile(ptr) };
  if print {
    pretty_print_release_info(&release_info);
  }
  Ok(release_info.version_string().to_owned())
}

fn patch(
  binary: &Path,
  version: &str,
  dry_run: bool,
  force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  let file = OpenOptions::new().read(true).write(true).open(binary)?;
  let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
  println!("Loaded {} bytes", mmap.len());
  let header = DENO_METADATA_BLOCK_SIGNATURE.to_le_bytes();
  let searcher = TwoWaySearcher::new(&header);
  let offset = searcher
    .search_in(&mmap)
    .ok_or("Failed to locate signature")?;
  println!("Found at offset {offset}");
  let ptr = unsafe { mmap.as_mut_ptr().add(offset) as *mut _ };
  let mut release_info: ReleaseInfo = unsafe { std::ptr::read_volatile(ptr) };
  if !force {
    if release_info.release_state == ReleaseState::Released {
      if release_info.version_string() == version {
        println!("Already patched (use --force to re-patch)");
        return Ok(());
      } else {
        return Err("Already released (use --force to force)".into());
      }
    }
  }
  release_info.release_state = ReleaseState::Released;
  release_info.version_string = [0; 16];
  release_info.version_string[..version.as_bytes().len()]
    .copy_from_slice(version.as_bytes());
  println!("Updating release info:");
  pretty_print_release_info(&release_info);

  if !dry_run {
    unsafe { std::ptr::write_volatile(ptr, release_info) }
    mmap.flush()?;
    drop(mmap);
    drop(file);
    UnifiedSigner
    println!("Validating...");
    let readback = read(binary, false)?;
    if readback != version {
      return Err(format!("Version mismatch {version} != {readback}!").into());
    }
    println!("Success!");
  }

  Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
  let args: Vec<String> = std::env::args().collect::<Vec<_>>();
  let args = match clap_root().try_get_matches_from_mut(args) {
    Ok(args) => args,
    Err(error) => {
      error.print().unwrap();
      return Err("".into());
    }
  };
  let (subcommand, args) = args.subcommand().unwrap();
  if subcommand == "patch" {
    let binary = args.get_one::<PathBuf>("binary").unwrap();
    let version = args.get_one::<String>("version").unwrap();
    let dry_run = args.get_flag("dry-run");
    let force = args.get_flag("force");
    patch(binary, version, dry_run, force)?;
  } else if subcommand == "read" {
    let binary = args.get_one::<PathBuf>("binary").unwrap();
    read(binary, true)?;
  }
  Ok(())
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
  run()
}
