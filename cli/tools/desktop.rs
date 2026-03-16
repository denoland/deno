// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;

use crate::args::CompileFlags;
use crate::args::DenoSubcommand;
use crate::args::DesktopFlags;
use crate::args::Flags;
use crate::factory::CliFactory;

pub async fn desktop(
  mut flags: Flags,
  desktop_flags: DesktopFlags,
) -> Result<(), AnyError> {
  let all_targets = desktop_flags.all_targets;

  // Read desktop config from deno.json to use as defaults.
  // CLI flags take precedence over config file values.
  let desktop_config = {
    let config_flags = flags.clone();
    let factory = CliFactory::from_flags(Arc::new(config_flags));
    let cli_options = factory.cli_options()?;
    cli_options.start_dir.to_desktop_config()?.clone()
  };

  // Resolve icon: CLI --icon > config icons (platform-specific)
  let icon = desktop_flags.icon.or_else(|| {
    desktop_config.app.as_ref().and_then(|app| {
      app.icons.as_ref().and_then(|icons| {
        if cfg!(target_os = "macos") {
          icons.macos.clone()
        } else if cfg!(target_os = "windows") {
          icons.windows.clone()
        } else {
          icons.linux.clone()
        }
      })
    })
  });

  // Resolve backend: CLI --backend > config backend
  let backend = desktop_flags
    .backend
    .or_else(|| desktop_config.backend.clone());

  // Resolve output: CLI --output > config output (platform-specific)
  let output = desktop_flags.output.or_else(|| {
    desktop_config.output.as_ref().and_then(|out| {
      if cfg!(target_os = "macos") {
        out.macos.clone()
      } else if cfg!(target_os = "windows") {
        out.windows.clone()
      } else {
        out.linux.clone()
      }
    })
  });

  // Use app name from config if no output specified
  let output = output
    .or_else(|| desktop_config.app.as_ref().and_then(|app| app.name.clone()));

  let compile_flags = CompileFlags {
    source_file: desktop_flags.source_file,
    output,
    args: desktop_flags.args,
    target: desktop_flags.target,
    no_terminal: false,
    icon,
    include: desktop_flags.include,
    exclude: desktop_flags.exclude,
    eszip: false,
    self_extracting: false,
    desktop: true,
    hmr: desktop_flags.hmr,
    backend,
  };

  // Update the subcommand in flags so compile internals see it correctly.
  flags.subcommand = DenoSubcommand::Compile(compile_flags.clone());

  if all_targets {
    let targets = [
      "x86_64-apple-darwin",
      "aarch64-apple-darwin",
      "x86_64-unknown-linux-gnu",
      "aarch64-unknown-linux-gnu",
      "x86_64-pc-windows-msvc",
    ];
    for target in targets {
      log::info!("Building for target: {}", target);
      let mut target_flags = compile_flags.clone();
      target_flags.target = Some(target.to_string());
      let mut target_outer_flags = flags.clone();
      target_outer_flags.subcommand =
        DenoSubcommand::Compile(target_flags.clone());
      super::compile::compile(target_outer_flags, target_flags).await?;
    }
    Ok(())
  } else {
    super::compile::compile(flags, compile_flags).await
  }
}
