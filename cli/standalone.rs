use deno_core::error::bail;
use deno_core::error::AnyError;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;

const MAGIC_TRAILER: &[u8; 8] = b"d3n0l4nd";

pub async fn create_standalone_binary(
  mut source_code: Vec<u8>,
  output: PathBuf,
) -> Result<(), AnyError> {
  let cli_binary = std::env::current_exe()?;
  let deno_rt_path = if cfg!(windows) {
    cli_binary.parent().unwrap().join("deno-rt.exe")
  } else {
    cli_binary.parent().unwrap().join("deno-rt")
  };

  let mut original_bin = tokio::fs::read(deno_rt_path).await?;

  let mut trailer = MAGIC_TRAILER.to_vec();
  trailer.write_all(&original_bin.len().to_be_bytes())?;

  let mut final_bin =
    Vec::with_capacity(original_bin.len() + source_code.len() + trailer.len());
  final_bin.append(&mut original_bin);
  final_bin.append(&mut source_code);
  final_bin.append(&mut trailer);

  let output =
    if cfg!(windows) && output.extension().unwrap_or_default() != "exe" {
      PathBuf::from(output.display().to_string() + ".exe")
    } else {
      output
    };

  if output.exists() {
    // If the output is a directory, throw error
    if output.is_dir() {
      bail!("Could not compile: {:?} is a directory.", &output);
    }

    // Make sure we don't overwrite any file not created by Deno compiler.
    // Check for magic trailer in last 16 bytes
    let mut output_file = File::open(&output)?;
    output_file.seek(SeekFrom::End(-16))?;
    let mut trailer = [0; 16];
    output_file.read_exact(&mut trailer)?;
    let (magic_trailer, _) = trailer.split_at(8);
    if magic_trailer != MAGIC_TRAILER {
      bail!("Could not compile: cannot overwrite {:?}.", &output);
    }
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
