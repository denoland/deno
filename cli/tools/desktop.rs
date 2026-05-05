// Copyright 2018-2026 the Deno authors. MIT license.

use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_terminal::colors;
use sha2::Digest;

use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::args::DenoSubcommand;
use crate::args::DesktopFlags;
use crate::args::Flags;
use crate::args::TypeCheckMode;
use crate::factory::CliFactory;
use crate::http_util::HttpClientProvider;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

/// Version of the `wef` capi crate pinned in the workspace Cargo.lock.
/// Populated by `cli/build.rs` and used to resolve matching prebuilt backend
/// binaries from `github.com/littledivy/just-wef/releases/tag/v{WEF_VERSION}`.
const WEF_VERSION: &str = env!("WEF_VERSION");

/// Rustc target triple the deno binary was built for. Used as the default
/// target when selecting a prebuilt wef backend archive.
const WEF_NATIVE_TARGET: &str = env!("TARGET");

/// Trust anchor for WEF backend downloads: SHA-256 digests of every archive
/// for the pinned `WEF_VERSION`. Checked into the repo so `SHA256SUMS` does
/// not need to be fetched (and trusted) at runtime — that file's integrity
/// previously rested on TOFU against the GitHub releases page. See
/// `cli/wef_sums.lock` for the format.
const WEF_PINNED_SUMS: &str = include_str!("../wef_sums.lock");

pub async fn desktop(
  flags: Flags,
  mut desktop_flags: DesktopFlags,
) -> Result<(), AnyError> {
  let all_targets = desktop_flags.all_targets;

  let config_flags = flags.clone();
  let factory = CliFactory::from_flags(Arc::new(config_flags));
  let cli_options = factory.cli_options()?;
  let desktop_config = cli_options.start_dir.to_desktop_config()?.clone();
  let wef_resolver = Arc::new(WefBackendResolver::new(&factory)?);

  if let Some(output) = desktop_config.output
    && desktop_flags.output.is_none()
  {
    desktop_flags.output = if cfg!(target_os = "macos") {
      output.macos
    } else if cfg!(target_os = "windows") {
      output.windows
    } else {
      output.linux
    };
  }

  if let Some(app_config) = desktop_config.app {
    if let Some(icons) = app_config.icons
      && desktop_flags.icon.is_none()
    {
      use deno_config::deno_json::DesktopIconValue;
      let platform_icon = if cfg!(target_os = "macos") {
        icons.macos
      } else if cfg!(target_os = "windows") {
        icons.windows
      } else {
        icons.linux
      };
      desktop_flags.icon = platform_icon.map(|v| match v {
        DesktopIconValue::Single(s) => crate::args::IconConfig::Single(s),
        DesktopIconValue::Set(entries) => crate::args::IconConfig::Set(
          entries
            .into_iter()
            .map(|e| crate::args::IconSetEntry {
              path: e.path,
              size: e.size,
            })
            .collect(),
        ),
      });
    }

    if let Some(name) = app_config.name
      && desktop_flags.output.is_none()
    {
      desktop_flags.output = Some(name);
    }
  }

  if let Some(backend) = desktop_config.backend
    && desktop_flags.backend.is_none()
  {
    desktop_flags.backend = Some(backend);
  }

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
      let mut desktop_flags = desktop_flags.clone();
      desktop_flags.target = Some(target.to_string());
      compile_desktop(flags.clone(), desktop_flags, cli_options, &wef_resolver)
        .await?;
    }
    Ok(())
  } else {
    compile_desktop(flags, desktop_flags, cli_options, &wef_resolver).await
  }
}

async fn compile_desktop(
  mut flags: Flags,
  mut desktop_flags: DesktopFlags,
  cli_options: &Arc<CliOptions>,
  wef_resolver: &WefBackendResolver,
) -> Result<(), AnyError> {
  // If the user asked for a `.dmg` (macOS) installer via `--output`, strip
  // the extension for the intermediate compile/bundle step and remember the
  // original so we can wrap the resulting .app in a DMG at the end.
  let dmg_output = desktop_flags
    .output
    .as_ref()
    .filter(|o| o.to_lowercase().ends_with(".dmg"))
    .cloned();
  if let Some(ref dmg) = dmg_output {
    if !cfg!(target_os = "macos") {
      bail!(
        "Building a .dmg requires a macOS build host (uses hdiutil). \
         Requested output: {dmg}. Build on macOS, or choose a different output \
         format.",
      );
    }
    let stem = Path::new(dmg)
      .file_stem()
      .map(|s| s.to_string_lossy().into_owned())
      .unwrap_or_else(|| "App".to_string());
    let parent = Path::new(dmg)
      .parent()
      .filter(|p| !p.as_os_str().is_empty());
    desktop_flags.output = Some(match parent {
      Some(p) => p.join(&stem).to_string_lossy().into_owned(),
      None => stem,
    });
  }

  // Same for `.AppImage` on Linux — strip extension, wrap app dir in an
  // AppImage at the end.
  let appimage_output = desktop_flags
    .output
    .as_ref()
    .filter(|o| o.to_lowercase().ends_with(".appimage"))
    .cloned();
  if let Some(ref appimage) = appimage_output {
    let stem = Path::new(appimage)
      .file_stem()
      .map(|s| s.to_string_lossy().into_owned())
      .unwrap_or_else(|| "App".to_string());
    let parent = Path::new(appimage)
      .parent()
      .filter(|p| !p.as_os_str().is_empty());
    desktop_flags.output = Some(match parent {
      Some(p) => p.join(&stem).to_string_lossy().into_owned(),
      None => stem,
    });
  }

  // Desktop framework detection: when --desktop is used and the source is
  // "." (a directory), detect the framework and generate the entrypoint.
  // The cwd resolved from CliOptions is reused for the HMR launch below so
  // framework detection is single-sourced and can't drift between the two.
  let detection_cwd = cli_options.initial_cwd().to_path_buf();
  let detected_framework = if desktop_flags.source_file == "." {
    super::framework::detect_framework(&detection_cwd)?
  } else {
    None
  };
  let _desktop_entrypoint_file = if desktop_flags.source_file == "." {
    let cwd = &detection_cwd;
    if let Some(detection) = detected_framework.as_ref() {
      let entrypoint_code = detection.entrypoint_code.clone();
      let includes = detection.include_paths.clone();
      log::info!("Detected {} framework", detection.name);
      // Enable CJS detection for Node-based frameworks.
      flags.unstable_config.detect_cjs = true;
      if detection.name == "Next.js"
        && !matches!(flags.type_check_mode, TypeCheckMode::None)
      {
        log::info!(
          "Disabling Deno type checking for Next.js desktop compile; Next handles app compilation itself"
        );
        flags.type_check_mode = TypeCheckMode::None;
      }
      // Write a temporary entrypoint file. tempfile gives us a unique
      // name (no collision between concurrent `deno desktop` runs in
      // the same project) and 0600 mode (no symlink-pre-creation
      // attack); cleanup-on-drop replaces the explicit guard.
      let entrypoint_temp = tempfile::Builder::new()
        .prefix(".deno_desktop_entry-")
        .suffix(".ts")
        .tempfile_in(cwd)
        .with_context(|| {
          format!("failed to create temp entrypoint file in {}", cwd.display())
        })?;
      {
        use std::io::Write;
        entrypoint_temp
          .as_file()
          .write_all(entrypoint_code.as_bytes())?;
      }
      let entrypoint_path = entrypoint_temp.path().to_path_buf();
      desktop_flags.source_file = entrypoint_path.display().to_string();
      if desktop_flags.output.is_none() {
        if let Some(dir_name) = cwd.file_name() {
          desktop_flags.output = Some(dir_name.to_string_lossy().into_owned());
        }
      }
      // Add framework build output to includes.
      for inc in includes {
        if !desktop_flags.include.contains(&inc) {
          desktop_flags.include.push(inc.clone());
        }
      }
      Some(entrypoint_temp)
    } else {
      bail!(
        "Could not detect a supported framework in the current directory.\nSupported frameworks: Next.js, Astro\nProvide an explicit entrypoint instead of \".\"."
      );
    }
  } else {
    None
  };

  let self_extracting = _desktop_entrypoint_file.is_some();
  // `_desktop_entrypoint_file` (a NamedTempFile) keeps the file alive
  // until end-of-scope and removes it on drop. Hold it past
  // compile_binary by keeping it bound here.

  let compile_flags = CompileFlags {
    source_file: desktop_flags.source_file.clone(),
    output: desktop_flags.output.clone(),
    args: desktop_flags.args.clone(),
    target: desktop_flags.target.clone(),
    no_terminal: false,
    icon: match &desktop_flags.icon {
      Some(crate::args::IconConfig::Single(s)) => Some(s.clone()),
      _ => None,
    },
    include: desktop_flags.include.clone(),
    exclude: desktop_flags.exclude.clone(),
    eszip: false,
    self_extracting,
  };

  let mut temp_flags = flags.clone();
  temp_flags.subcommand = DenoSubcommand::Compile(compile_flags.clone());
  temp_flags.internal.is_desktop = true;

  let output_path =
    super::compile::compile_binary(Arc::new(temp_flags), compile_flags, true)
      .await?;

  let inspector_requested = flags.inspect.is_some()
    || flags.inspect_brk.is_some()
    || flags.inspect_wait.is_some();

  if desktop_flags.hmr || inspector_requested {
    let backend = desktop_flags.backend.as_deref().unwrap_or("webview");
    run_desktop_hmr(
      &output_path,
      &detection_cwd,
      detected_framework.as_ref(),
      backend,
      wef_resolver,
      &flags,
      &desktop_flags,
    )
    .await?;
  } else {
    // Package the dylib into a platform-specific app bundle.
    let bundle_path = package_desktop_app(
      &output_path,
      &desktop_flags,
      cli_options,
      wef_resolver,
    )
    .await?;

    // If the user requested a .dmg, wrap the .app in one and report the DMG.
    // If the user requested a .AppImage, wrap the Linux app dir in one.
    let final_path = if let Some(dmg) = dmg_output.as_deref() {
      let dmg_abs = cli_options.initial_cwd().join(dmg);
      create_macos_dmg(&bundle_path, &dmg_abs)?;
      dmg_abs
    } else if let Some(appimage) = appimage_output.as_deref() {
      let appimage_abs = cli_options.initial_cwd().join(appimage);
      create_linux_appimage(
        &bundle_path,
        &appimage_abs,
        desktop_flags.target.as_deref(),
      )?;
      appimage_abs
    } else {
      bundle_path
    };

    let initial_cwd =
      deno_path_util::url_from_directory_path(cli_options.initial_cwd())?;
    log::info!(
      "{} {}",
      colors::green("Bundle"),
      if let Ok(bundle_url) = deno_path_util::url_from_file_path(&final_path) {
        crate::util::path::relative_specifier_path_for_display(
          &initial_cwd,
          &bundle_url,
        )
      } else {
        final_path.display().to_string()
      }
    );
  }

  Ok(())
}

/// Launch the desktop app with HMR enabled after compilation.
///
/// Framework dev servers provide HMR via websocket. Since they run inside
/// the Deno desktop runtime, `Deno.desktop` APIs remain available.
/// `child_process.fork()` works because forked workers use
/// `override_main_module` to run the target script instead of the
/// embedded entrypoint.
async fn run_desktop_hmr(
  dylib_path: &Path,
  source_dir: &Path,
  framework: Option<&super::framework::FrameworkDetection>,
  backend: &str,
  wef_resolver: &WefBackendResolver,
  flags: &Flags,
  desktop_flags: &DesktopFlags,
) -> Result<(), AnyError> {
  let wef_backend =
    wef_resolver.find_binary(backend, WEF_NATIVE_TARGET).await?;
  let dylib_abs = dylib_path
    .canonicalize()
    .unwrap_or(dylib_path.to_path_buf());
  let source_abs = source_dir
    .canonicalize()
    .unwrap_or(source_dir.to_path_buf());

  if let Some(fw) = framework
    && desktop_flags.hmr
  {
    log::info!(
      "{} {} dev server with HMR in desktop mode",
      colors::green("Running"),
      fw.name,
    );
  }

  if desktop_flags.hmr {
    log::info!(
      "{} desktop app with HMR (watching {})",
      colors::green("Running"),
      source_abs.display(),
    );
  } else {
    log::info!("{} desktop app under inspector", colors::green("Running"),);
  }

  let mut cmd = std::process::Command::new(&wef_backend);
  cmd
    .arg("--runtime")
    .arg(&dylib_abs)
    .env("WEF_RUNTIME_PATH", &dylib_abs)
    .current_dir(&source_abs);
  // Only enable the file watcher + setScriptSource pipeline when the user
  // actually asked for HMR. `deno desktop --inspect` alone used to spin up
  // both, surprising users (and burning the inspector channel on hot
  // reloads they didn't request).
  if desktop_flags.hmr {
    cmd.env("DENO_DESKTOP_HMR", &source_abs);
  }

  // Wire up the unified DevTools multiplexer when --inspect is set.
  // The mux runs in this (parent) process and fronts both the Deno runtime
  // inspector (in the WEF subprocess) and the CEF renderer's debug port
  // (in CEF's child process). We allocate two internal ports here, hand
  // them to the subprocess via env vars, and bind the user-visible port
  // for DevTools to attach to.
  let user_inspect = flags.inspect.or(flags.inspect_brk).or(flags.inspect_wait);
  let mux_handle = if let Some(user_addr) = user_inspect {
    let deno_internal: SocketAddr = format!(
      "127.0.0.1:{}",
      crate::tools::desktop_devtools::allocate_random_port()?
    )
    .parse()
    .unwrap();
    let cef_internal: SocketAddr = match desktop_flags.inspect_renderer {
      Some(addr) => addr,
      None => format!(
        "127.0.0.1:{}",
        crate::tools::desktop_devtools::allocate_random_port()?
      )
      .parse()
      .unwrap(),
    };
    let wait_for_debugger =
      flags.inspect_brk.is_some() || flags.inspect_wait.is_some();
    let handle = crate::tools::desktop_devtools::spawn_mux(
      crate::tools::desktop_devtools::MuxConfig {
        listen: user_addr,
        deno_internal,
        cef_internal,
        inspect_brk: flags.inspect_brk.is_some(),
        wait_for_debugger,
      },
    )
    .await?;

    log::info!(
      "{} DevTools on ws://{}  (open chrome://inspect)",
      colors::green("Inspector"),
      handle.listen,
    );
    log::debug!(
      "[desktop] internal upstream ports: deno={} cef={}",
      deno_internal,
      cef_internal,
    );

    cmd
      .env(
        "DENO_DESKTOP_INSPECT_INTERNAL_PORT",
        deno_internal.to_string(),
      )
      // Exposed so rt_desktop's `openDevtools()` can launch a browser
      // pointed at the unified DevTools frontend instead of CEF's
      // renderer-only native window.
      .env("DENO_DESKTOP_MUX_WS", handle.listen.to_string())
      .env("WEF_REMOTE_DEBUGGING_PORT", cef_internal.port().to_string());
    if flags.inspect_brk.is_some() {
      cmd.env("DENO_DESKTOP_INSPECT_BRK", "1");
    }
    if flags.inspect_wait.is_some() {
      cmd.env("DENO_DESKTOP_INSPECT_WAIT", "1");
    }
    Some(handle)
  } else {
    None
  };

  // `kill_on_drop` is a safety net: if the parent panics or exits via any
  // path that doesn't reach the explicit `wait` below, the WEF backend
  // (and its CEF renderer subprocesses) get SIGKILLed on `Child` drop
  // rather than being orphaned. Normal Ctrl-C delivers SIGINT to the
  // whole process group so this rarely matters in practice; it covers
  // the abnormal-exit cases.
  let mut child = tokio::process::Command::from(cmd)
    .kill_on_drop(true)
    .spawn()
    .with_context(|| {
      format!("Failed to launch WEF backend: {}", wef_backend.display())
    })?;

  let status = child
    .wait()
    .await
    .context("Failed waiting for WEF backend")?;

  // Keep the mux alive until the subprocess exits, then drop it.
  drop(mux_handle);

  if !status.success() {
    bail!("WEF backend exited with status: {}", status);
  }
  Ok(())
}

/// Package a compiled desktop dylib into a platform-specific app bundle.
async fn package_desktop_app(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  wef_resolver: &WefBackendResolver,
) -> Result<PathBuf, AnyError> {
  let target = desktop_flags.target.as_deref();
  let is_darwin = match target {
    Some(target) => target.contains("darwin"),
    None => cfg!(target_os = "macos"),
  };
  let is_windows = match target {
    Some(target) => target.contains("windows"),
    None => cfg!(target_os = "windows"),
  };

  if is_darwin {
    package_macos_app_bundle(
      dylib_path,
      desktop_flags,
      cli_options,
      wef_resolver,
    )
    .await
  } else if is_windows {
    package_windows_app_dir(
      dylib_path,
      desktop_flags,
      cli_options,
      wef_resolver,
    )
    .await
  } else {
    package_linux_app_dir(dylib_path, desktop_flags, cli_options, wef_resolver)
      .await
  }
}

/// Create a Windows app directory from the compiled desktop dylib.
///
/// Directory structure:
/// ```text
/// AppName/
///   AppName.bat         (launcher)
///   wef.exe             (WEF backend binary)
///   libcef.dll, ...     (CEF support files, if any)
///   denort.dll          (compiled Deno runtime + user code)
///   AppIcon.ico         (optional)
/// ```
async fn package_windows_app_dir(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  wef_resolver: &WefBackendResolver,
) -> Result<PathBuf, AnyError> {
  let parts = dylib_parts(dylib_path)?;
  let app_name = parts.app_name;
  let app_dir = parts.parent.join(&app_name);

  let backend = desktop_flags.backend.as_deref().unwrap_or("cef");
  let target = wef_target_for(desktop_flags);
  let wef_binary = wef_resolver.find_binary(backend, target).await?;
  let wef_dir = wef_resolver.find_binary_dir(backend, target).await?;
  let wef_binary_name = wef_binary
    .file_name()
    .unwrap()
    .to_string_lossy()
    .to_string();

  if app_dir.exists() {
    std::fs::remove_dir_all(&app_dir)?;
  }

  // Copy WEF backend directory (binary + CEF support files) as the shell.
  crate::tools::compile::copy_dir_all(&wef_dir, &app_dir)?;

  // Drop any self-extracting runtime cache dir that tagged along.
  let wef_exe_stem = Path::new(&wef_binary_name)
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| wef_binary_name.clone());
  let cache_dir = app_dir.join(format!(".{}", wef_exe_stem));
  if cache_dir.exists() {
    let _ = std::fs::remove_dir_all(&cache_dir);
  }
  let cache_file = app_dir.join(format!(".{}.cache", wef_exe_stem));
  if cache_file.exists() {
    let _ = std::fs::remove_file(&cache_file);
  }

  // Copy the compiled dylib (denort.dll) alongside the backend binary.
  let dylib_filename = parts.file_name;
  let dest_dylib = app_dir.join(dylib_filename);
  std::fs::copy(dylib_path, &dest_dylib)?;

  // Create a .bat launcher that invokes the backend with --runtime.
  // Validate every name we interpolate: cmd.exe expands `%VAR%` and
  // treats `^` `&` etc. as command separators even inside `"..."`.
  let dylib_filename_str = dylib_filename.to_string_lossy();
  validate_launcher_name(&app_name, "app name")?;
  validate_launcher_name(&wef_binary_name, "WEF backend binary name")?;
  validate_launcher_name(&dylib_filename_str, "dylib filename")?;
  let launcher_path = app_dir.join(format!("{}.bat", app_name));
  std::fs::write(
    &launcher_path,
    format!(
      "@echo off\r\n\
       set DIR=%~dp0\r\n\
       \"%DIR%{wef_binary}\" --runtime \"%DIR%{dylib}\" %*\r\n",
      wef_binary = wef_binary_name,
      dylib = dylib_filename_str,
    ),
  )?;

  // Handle icon — drop an .ico next to the launcher. Embedding the icon
  // into the .exe itself requires rcedit or equivalent and is out of scope.
  if let Some(ref icon) = desktop_flags.icon {
    let dest = app_dir.join("AppIcon.ico");
    match icon {
      crate::args::IconConfig::Single(path) => {
        let icon_path = cli_options.initial_cwd().join(path);
        if icon_path.exists() {
          match icon_path.extension().and_then(|e| e.to_str()) {
            Some("ico") => {
              std::fs::copy(&icon_path, &dest)?;
            }
            _ => {
              log::warn!(
                "Icon '{}' is not .ico, skipping",
                icon_path.display()
              );
            }
          }
        } else {
          log::warn!("Icon '{}' not found, skipping", icon_path.display());
        }
      }
      crate::args::IconConfig::Set(entries) => {
        convert_icon_set_to_ico(cli_options.initial_cwd(), entries, &dest)?;
      }
    }
  }

  // Remove the standalone dylib (it's now inside the app dir).
  let _ = std::fs::remove_file(dylib_path);

  Ok(app_dir)
}

/// Create a Linux app directory from the compiled desktop dylib.
///
/// Directory structure:
/// ```text
/// AppName/
///   AppName             (launcher shell script)
///   wef                 (WEF backend binary)
///   libcef.so, ...      (CEF support files, if any)
///   libdenort.so        (compiled Deno runtime + user code)
///   AppIcon.png         (optional)
/// ```
async fn package_linux_app_dir(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  wef_resolver: &WefBackendResolver,
) -> Result<PathBuf, AnyError> {
  let parts = dylib_parts(dylib_path)?;
  // `file_stem` on "libdenort.so" returns "libdenort" — strip the "lib" prefix
  // so the app directory is named after the app, not the runtime library.
  let app_name = parts
    .app_name
    .strip_prefix("lib")
    .map(|s| s.to_string())
    .unwrap_or(parts.app_name);
  let app_dir = parts.parent.join(&app_name);

  let backend = desktop_flags.backend.as_deref().unwrap_or("cef");
  let target = wef_target_for(desktop_flags);
  let wef_binary = wef_resolver.find_binary(backend, target).await?;
  let wef_dir = wef_resolver.find_binary_dir(backend, target).await?;
  let wef_binary_name = wef_binary
    .file_name()
    .unwrap()
    .to_string_lossy()
    .to_string();

  if app_dir.exists() {
    std::fs::remove_dir_all(&app_dir)?;
  }

  // Copy WEF backend directory (binary + CEF support files) as the shell.
  crate::tools::compile::copy_dir_all(&wef_dir, &app_dir)?;

  // Drop any self-extracting runtime cache dir that tagged along.
  let wef_exe_stem = Path::new(&wef_binary_name)
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| wef_binary_name.clone());
  let cache_dir = app_dir.join(format!(".{}", wef_exe_stem));
  if cache_dir.exists() {
    let _ = std::fs::remove_dir_all(&cache_dir);
  }
  let cache_file = app_dir.join(format!(".{}.cache", wef_exe_stem));
  if cache_file.exists() {
    let _ = std::fs::remove_file(&cache_file);
  }

  // Copy the compiled dylib alongside the backend binary.
  let dylib_filename = parts.file_name;
  let dest_dylib = app_dir.join(dylib_filename);
  std::fs::copy(dylib_path, &dest_dylib)?;

  // Create a shell launcher that invokes the backend with --runtime.
  // --ozone-platform=x11 forces CEF to create X11 windows (via XWayland on
  // Wayland sessions). The Linux WEF mouse/focus/resize event monitor uses
  // XI2 on X11 and does not support Wayland.
  // GDK_BACKEND=x11 aligns GDK with Ozone so GDK_IS_X11_DISPLAY is true.
  //
  // Validate every name we interpolate: bash expands `$VAR`, backticks,
  // and `$(...)` even inside `"..."`.
  let dylib_filename_str = dylib_filename.to_string_lossy();
  validate_launcher_name(&app_name, "app name")?;
  validate_launcher_name(&wef_binary_name, "WEF backend binary name")?;
  validate_launcher_name(&dylib_filename_str, "dylib filename")?;
  let launcher_path = app_dir.join(&app_name);
  std::fs::write(
    &launcher_path,
    format!(
      "#!/bin/sh\n\
       DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
       export GDK_BACKEND=x11\n\
       exec \"$DIR/{wef_binary}\" --ozone-platform=x11 --runtime \"$DIR/{dylib}\" \"$@\"\n",
      wef_binary = wef_binary_name,
      dylib = dylib_filename_str,
    ),
  )?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
      &launcher_path,
      std::fs::Permissions::from_mode(0o755),
    )?;
  }

  // Handle icon — copy a .png next to the launcher.
  if let Some(ref icon) = desktop_flags.icon {
    let dest = app_dir.join("AppIcon.png");
    match icon {
      crate::args::IconConfig::Single(path) => {
        let icon_path = cli_options.initial_cwd().join(path);
        if icon_path.exists() {
          match icon_path.extension().and_then(|e| e.to_str()) {
            Some("png") => {
              std::fs::copy(&icon_path, &dest)?;
            }
            _ => {
              log::warn!(
                "Icon '{}' is not .png, skipping",
                icon_path.display()
              );
            }
          }
        } else {
          log::warn!("Icon '{}' not found, skipping", icon_path.display());
        }
      }
      crate::args::IconConfig::Set(entries) => {
        // Pick the largest provided size as the single icon file.
        if let Some(largest) = entries.iter().max_by_key(|e| e.size) {
          let src = cli_options.initial_cwd().join(&largest.path);
          if src.exists() {
            std::fs::copy(&src, &dest)?;
          } else {
            log::warn!("Icon '{}' not found, skipping", src.display());
          }
        }
      }
    }
  }

  // Remove the standalone dylib (it's now inside the app dir).
  let _ = std::fs::remove_file(dylib_path);

  Ok(app_dir)
}

/// Environment variable pointing at a local wef checkout, used to bypass the
/// download path during development. Build-tree subpaths under this directory
/// are searched the same way the old sibling-checkout heuristic searched.
const WEF_DEV_DIR_ENV: &str = "WEF_DEV_DIR";

/// Resolves WEF backend binaries and `.app` bundles, falling back to
/// downloading prebuilt archives from the wef GitHub releases when
/// `WEF_DEV_DIR` is not set.
struct WefBackendResolver {
  http_client_provider: Arc<HttpClientProvider>,
  /// `<deno_dir>/wef/<version>/`
  cache_root: PathBuf,
}

impl WefBackendResolver {
  fn new(factory: &CliFactory) -> Result<Self, AnyError> {
    let cache_root = factory.deno_dir()?.root.join("wef").join(WEF_VERSION);
    Ok(Self {
      http_client_provider: factory.http_client_provider().clone(),
      cache_root,
    })
  }

  fn backend_cache_dir(&self, backend: &str, target: &str) -> PathBuf {
    self.cache_root.join(backend).join(target)
  }

  /// Download + verify + extract a backend archive if it isn't already in
  /// `<deno_dir>/wef/<version>/<backend>/<target>/`.
  async fn ensure_downloaded(
    &self,
    backend: &str,
    target: &str,
  ) -> Result<PathBuf, AnyError> {
    let dir = self.backend_cache_dir(backend, target);
    let marker = dir.join(".downloaded");
    if marker.exists() {
      return Ok(dir);
    }

    let archive = wef_archive_name(backend, target);
    let client = self.http_client_provider.get_or_create()?;

    // Use the in-tree pinned digests rather than fetching SHA256SUMS from the
    // release page. The latter is unsigned, so trusting it would let anyone
    // who can write to the wef release host swap both archive and sums
    // together (TOFU). The lock file is reviewed in PRs when WEF_VERSION
    // bumps, so this is the trust anchor.
    check_pinned_sums_version()?;
    let expected = parse_sha256sum(WEF_PINNED_SUMS, &archive).ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "no pinned SHA-256 for {archive} in cli/wef_sums.lock \
         (regenerate when bumping WEF_VERSION to v{WEF_VERSION}; \
         wef v{WEF_VERSION} release may not include backend '{backend}' for target '{target}')"
      )
    })?;

    log::info!(
      "{} wef {} backend for {} (v{})",
      colors::green("Downloading"),
      backend,
      target,
      WEF_VERSION,
    );

    let url = Url::parse(&wef_release_url(&archive))?;
    let progress_bar = ProgressBar::new(ProgressBarStyle::DownloadBars);
    let progress = progress_bar.update(&archive);
    // Send a real User-Agent — some CDNs (incl. parts of GitHub
    // releases) start rate-limiting empty UAs aggressively.
    let mut headers = http::HeaderMap::new();
    if let Ok(ua) = http::HeaderValue::from_str(&format!(
      "deno-desktop/{} (+https://deno.com)",
      env!("CARGO_PKG_VERSION")
    )) {
      headers.insert(http::header::USER_AGENT, ua);
    }
    let response = client
      .download_with_progress_and_retries(url.clone(), &headers, &progress)
      .await
      .with_context(|| format!("failed to download {url}"))?;
    let data = response
      .into_maybe_bytes()?
      .ok_or_else(|| deno_core::anyhow::anyhow!("empty response from {url}"))?;

    let actual =
      faster_hex::hex_string(&sha2::Sha256::digest(&data)).to_lowercase();
    let expected_lc = expected.to_lowercase();
    if actual != expected_lc {
      // Include the URL in the bail: an attacker who poisoned a redirect
      // would otherwise be invisible in the failure log.
      bail!(
        "checksum mismatch for {archive} (downloaded from {url})\n  expected: {expected_lc}\n  actual:   {actual}"
      );
    }

    let parent = dir.parent().ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "WEF cache dir has no parent: {}",
        dir.display()
      )
    })?;
    std::fs::create_dir_all(parent)?;

    // Stage extraction in a sibling tempdir so concurrent `deno desktop`
    // builds don't see (or stomp on) a half-populated `dir` while one
    // is mid-extract. tempfile's cleanup-on-drop covers panic /
    // early-return paths; on the happy path we consume the TempDir via
    // `into_path` so the rename below sees a real directory.
    let staging = tempfile::Builder::new()
      .prefix(".staging-")
      .tempdir_in(parent)
      .with_context(|| {
        format!("failed to stage tempdir in {}", parent.display())
      })?;

    extract_wef_archive(&archive, &data, staging.path())
      .with_context(|| format!("failed to extract {archive}"))?;
    // Marker is written into the staging dir so it lands atomically with
    // the rest of the contents — a SIGKILL during extraction can never
    // leave a marker-without-payload state.
    std::fs::write(
      staging.path().join(".downloaded"),
      format!("v{WEF_VERSION}\n"),
    )?;

    // Another process may have raced us and finished its extract while
    // we were downloading. Use theirs and let staging clean up on drop.
    if marker.exists() {
      return Ok(dir);
    }

    // Discard any prior failed-extract debris (no marker ⇒ partial).
    // `rename` refuses a non-empty target.
    if dir.exists() {
      std::fs::remove_dir_all(&dir)?;
    }

    let staging_path = staging.into_path();
    if let Err(e) = std::fs::rename(&staging_path, &dir) {
      // Lost a rename race with a concurrent process — its dir is at
      // our target path now. Drop our staged copy and use theirs.
      let _ = std::fs::remove_dir_all(&staging_path);
      if marker.exists() {
        return Ok(dir);
      }
      return Err(deno_core::anyhow::anyhow!(
        "failed to atomic-rename WEF cache to {}: {e}",
        dir.display(),
      ));
    }

    Ok(dir)
  }

  /// Locate the WEF backend binary for `backend` on `target`.
  ///
  /// Resolution order: `WEF_DEV_DIR` checkout → cached download →
  /// fresh download.
  async fn find_binary(
    &self,
    backend: &str,
    target: &str,
  ) -> Result<PathBuf, AnyError> {
    if let Some(dev_dir) = wef_dev_dir() {
      return locate_dev_backend_binary(&dev_dir, backend).ok_or_else(|| {
        deno_core::anyhow::anyhow!(
          "could not find '{backend}' backend binary under {} (set via {})",
          dev_dir.display(),
          WEF_DEV_DIR_ENV
        )
      });
    }

    let dir = self.ensure_downloaded(backend, target).await?;
    locate_backend_binary(&dir, backend, target).ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "could not find '{backend}' backend binary inside {}",
        dir.display()
      )
    })
  }

  /// Locate the WEF `.app` bundle for `backend` on a macOS `target`.
  async fn find_app_bundle(
    &self,
    backend: &str,
    target: &str,
  ) -> Result<PathBuf, AnyError> {
    if let Some(dev_dir) = wef_dev_dir() {
      return locate_dev_app_bundle(&dev_dir, backend).ok_or_else(|| {
        deno_core::anyhow::anyhow!(
          "could not find '{backend}' .app bundle under {} (set via {})",
          dev_dir.display(),
          WEF_DEV_DIR_ENV
        )
      });
    }

    let dir = self.ensure_downloaded(backend, target).await?;
    locate_app_bundle(&dir, backend).ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "could not find '{backend}' .app bundle inside {} (backend may not ship as an app for target '{target}')",
        dir.display()
      )
    })
  }

  /// Directory containing the backend binary and its support files (used on
  /// Windows / Linux where support files sit alongside the binary).
  async fn find_binary_dir(
    &self,
    backend: &str,
    target: &str,
  ) -> Result<PathBuf, AnyError> {
    let binary = self.find_binary(backend, target).await?;
    let parent = binary.parent().ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "WEF backend binary has no parent directory: {}",
        binary.display()
      )
    })?;
    Ok(parent.to_path_buf())
  }
}

fn wef_archive_name(backend: &str, target: &str) -> String {
  let ext = if target.contains("windows") {
    "zip"
  } else {
    "tar.gz"
  };
  // The `raw` backend ships under the `winit` archive name upstream.
  let archive_backend = match backend {
    "raw" => "winit",
    other => other,
  };
  format!("wef-{archive_backend}-{target}.{ext}")
}

fn wef_release_url(file: &str) -> String {
  format!(
    "https://github.com/littledivy/just-wef/releases/download/v{WEF_VERSION}/{file}"
  )
}

/// Confirm the pinned digests file targets the same WEF_VERSION the binary
/// was built against. The lock file optionally carries a `# version: vX.Y.Z`
/// directive; when present it must match `WEF_VERSION`.
fn check_pinned_sums_version() -> Result<(), AnyError> {
  for line in WEF_PINNED_SUMS.lines() {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('#') else {
      continue;
    };
    let rest = rest.trim();
    let Some(version) = rest.strip_prefix("version:") else {
      continue;
    };
    let pinned = version.trim().trim_start_matches('v');
    if pinned.is_empty() {
      bail!(
        "cli/wef_sums.lock has no pinned WEF version — populate it for v{WEF_VERSION} before downloading wef backends"
      );
    }
    if pinned != WEF_VERSION {
      bail!(
        "cli/wef_sums.lock pins WEF v{pinned} but this build expects v{WEF_VERSION} — refresh the lock file"
      );
    }
    return Ok(());
  }
  Ok(())
}

/// Pick out the hex digest for `file` from a GNU `sha256sum`-style file. Each
/// line is `<hex>  <filename>` (optionally `<hex>  *<filename>` for binary
/// mode).
fn parse_sha256sum(contents: &str, file: &str) -> Option<String> {
  for line in contents.lines() {
    let mut parts = line.split_whitespace();
    let hex = parts.next()?;
    let name = parts.next()?;
    if name.trim_start_matches('*') == file {
      return Some(hex.to_string());
    }
  }
  None
}

fn extract_wef_archive(
  name: &str,
  data: &[u8],
  dest: &Path,
) -> Result<(), AnyError> {
  if name.ends_with(".tar.gz") {
    let decoder = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(decoder);
    // Strip mode bits from archive entries: a tampered archive could
    // otherwise ship setuid/setgid binaries into the deno cache. Files keep
    // the umask-applied default; we re-add execute bits below for entries
    // that need them.
    archive.set_preserve_permissions(false);
    for entry in archive.entries()? {
      let mut entry = entry?;
      let entry_path = entry.path()?.into_owned();
      // Defence in depth — pre-check `..` / root before handing to
      // `unpack_in`, since we want a hard error rather than the silent
      // skip that `unpack_in` does for a rejected entry.
      if entry_path.components().any(|c| {
        matches!(
          c,
          std::path::Component::ParentDir | std::path::Component::RootDir
        )
      }) {
        bail!(
          "refusing tar entry with traversal path: {}",
          entry_path.display()
        );
      }
      // `unpack_in` (vs. `unpack(absolute_path)`) makes tar enforce its
      // symlink + hardlink target containment too: a tar with entry A as
      // symlink `foo -> ../../etc` followed by entry B writing
      // `foo/passwd` would otherwise escape `dest`.
      if !entry.unpack_in(dest)? {
        bail!(
          "refusing tar entry that would unpack outside dest: {}",
          entry_path.display()
        );
      }
      let dest_path = dest.join(&entry_path);
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        // `symlink_metadata` so we don't follow a just-extracted symlink
        // and chmod its target.
        if let Ok(meta) = std::fs::symlink_metadata(&dest_path) {
          if meta.file_type().is_file() {
            // Was the entry executable? If so, mask to 0o755; otherwise 0o644.
            let mode = entry.header().mode().unwrap_or(0o644);
            let safe = if mode & 0o111 != 0 { 0o755 } else { 0o644 };
            let mut perms = meta.permissions();
            perms.set_mode(safe);
            let _ = std::fs::set_permissions(&dest_path, perms);
          }
        }
      }
    }
  } else if name.ends_with(".zip") {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))?;
    // Iterate entries manually rather than `archive.extract(dest)`: that
    // helper has the same shape as the tar `unpack` we deliberately
    // avoided (no perm masking; no defence-in-depth against zip-slip
    // beyond the crate's own checks). Treat the archive as untrusted.
    for i in 0..archive.len() {
      let mut entry = archive.by_index(i)?;
      // `enclosed_name` rejects drive labels, absolute paths and `..`
      // components. Anything that fails this check is a zip-slip attempt
      // (or a legitimately weird archive we don't want to handle).
      let Some(rel_path) = entry.enclosed_name() else {
        bail!("refusing zip entry with unsafe path: {}", entry.name());
      };
      // Defence in depth — re-check the components ourselves.
      if rel_path.components().any(|c| {
        matches!(
          c,
          std::path::Component::ParentDir | std::path::Component::RootDir
        )
      }) {
        bail!(
          "refusing zip entry with traversal path: {}",
          rel_path.display()
        );
      }
      // Refuse symlinks: with prior entries already extracted, a
      // symlink-then-write pair is the standard zip-slip-via-symlink
      // escape, and WEF Windows archives have no legitimate need for
      // them.
      if entry.is_symlink() {
        bail!(
          "refusing symlink entry in wef archive: {}",
          rel_path.display()
        );
      }
      let dest_path = dest.join(&rel_path);
      if entry.is_dir() {
        std::fs::create_dir_all(&dest_path)?;
        continue;
      }
      if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
      }
      let mut out = std::fs::File::create(&dest_path)?;
      std::io::copy(&mut entry, &mut out)?;
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        // Mask to 0o755 / 0o644 — same policy as the tar branch.
        // setuid/setgid/sticky bits are dropped; world-writable bits too.
        let mode = entry.unix_mode().unwrap_or(0o644);
        let safe = if mode & 0o111 != 0 { 0o755 } else { 0o644 };
        let _ = std::fs::set_permissions(
          &dest_path,
          std::fs::Permissions::from_mode(safe),
        );
      }
    }
  } else {
    bail!("unsupported archive format: {name}");
  }
  Ok(())
}

/// Resolve the backend binary path inside an extracted archive directory.
fn locate_backend_binary(
  dir: &Path,
  backend: &str,
  target: &str,
) -> Option<PathBuf> {
  let is_windows = target.contains("windows");
  let is_macos = target.contains("apple-darwin");
  match backend {
    "cef" if is_macos => {
      let p = dir.join("wef.app/Contents/MacOS/wef");
      p.exists().then_some(p)
    }
    "webview" if is_macos => {
      let p = dir.join("wef_webview.app/Contents/MacOS/wef_webview");
      p.exists().then_some(p)
    }
    _ => {
      let stem = match backend {
        "cef" => "wef",
        "raw" => "wef_winit",
        "servo" => "wef_servo",
        _ => "wef_webview",
      };
      let exe = if is_windows {
        format!("{stem}.exe")
      } else {
        stem.to_string()
      };
      let p = dir.join(&exe);
      p.exists().then_some(p)
    }
  }
}

fn locate_app_bundle(dir: &Path, backend: &str) -> Option<PathBuf> {
  let name = match backend {
    "cef" => "wef.app",
    _ => "wef_webview.app",
  };
  let p = dir.join(name);
  p.exists().then_some(p)
}

/// Target triple to use when selecting a wef backend archive. Honors
/// `desktop_flags.target` (for cross-target packaging); otherwise defaults to
/// the host triple this deno binary was built for.
fn wef_target_for(desktop_flags: &DesktopFlags) -> &str {
  desktop_flags.target.as_deref().unwrap_or(WEF_NATIVE_TARGET)
}

/// Resolve `WEF_DEV_DIR` to a directory path if set and present on disk.
fn wef_dev_dir() -> Option<PathBuf> {
  let raw = std::env::var(WEF_DEV_DIR_ENV).ok()?;
  let p = PathBuf::from(raw);
  p.is_dir().then_some(p)
}

/// Find a built backend binary inside a wef checkout. Mirrors the well-known
/// build-tree paths produced by wef's Makefile + Nix flakes.
fn locate_dev_backend_binary(wef: &Path, backend: &str) -> Option<PathBuf> {
  let candidates: Vec<PathBuf> = match backend {
    "cef" => vec![
      wef.join("result-cef/Applications/wef.app/Contents/MacOS/wef"),
      wef.join("result/Applications/wef.app/Contents/MacOS/wef"),
      wef.join("cef/build/Release/wef.app/Contents/MacOS/wef"),
      wef.join("cef/build/wef.app/Contents/MacOS/wef"),
      wef.join("cef/build/Release/wef"),
      wef.join("cef/build/wef"),
    ],
    "servo" => vec![
      wef.join("target/release/wef_servo"),
      wef.join("target/debug/wef_servo"),
    ],
    "raw" => vec![
      wef.join("target/release/wef_winit"),
      wef.join("target/debug/wef_winit"),
    ],
    _ => vec![
      wef.join(
        "result-1/Applications/wef_webview.app/Contents/MacOS/wef_webview",
      ),
      wef
        .join("result/Applications/wef_webview.app/Contents/MacOS/wef_webview"),
      wef.join("webview/build/wef_webview.app/Contents/MacOS/wef_webview"),
      wef.join("webview/build/wef_webview"),
    ],
  };
  candidates.into_iter().find(|p| p.exists())
}

/// Find a built backend `.app` bundle inside a wef checkout.
fn locate_dev_app_bundle(wef: &Path, backend: &str) -> Option<PathBuf> {
  let candidates: Vec<PathBuf> = match backend {
    "cef" => vec![
      wef.join("result-cef/Applications/wef.app"),
      wef.join("result/Applications/wef.app"),
      wef.join("cef/build/Release/wef.app"),
      wef.join("cef/build/wef.app"),
    ],
    "raw" | "servo" => return None,
    _ => vec![
      wef.join("result-1/Applications/wef_webview.app"),
      wef.join("result/Applications/wef_webview.app"),
      wef.join("webview/build/wef_webview.app"),
    ],
  };
  candidates.into_iter().find(|p| p.exists())
}

/// Read a top-level string from a plist file (XML or binary).
///
/// Uses the `plist` crate so a hostile or just-non-trivial Info.plist
/// (CDATA, entities, binary plist format, key reordered, etc.) parses
/// correctly — the previous string-scan implementation could be tricked
/// or silently mis-extract. Returns `None` on any read or parse failure.
fn read_plist_string(path: &Path, key: &str) -> Option<String> {
  let dict: plist::Dictionary = plist::from_file(path).ok()?;
  dict.get(key)?.as_string().map(|s| s.to_string())
}

/// Reject any name we'd interpolate into a generated launcher script
/// (POSIX shell on macOS/Linux, `.bat` on Windows). Even the
/// double-quoted positions take expansions: `$`, backticks, `\` in
/// bash; `%` and `^` in cmd.exe. The launcher kind context (`kind`)
/// is included in the error to make the failure easy to act on.
fn validate_launcher_name(name: &str, kind: &str) -> Result<(), AnyError> {
  if name.is_empty() {
    bail!("invalid {kind}: name is empty");
  }
  // ASCII-only, alphanumerics + a small whitelist of harmless
  // punctuation. Spaces are allowed because real macOS .app bundles
  // commonly have spaces in their executable names.
  let bad = name.chars().find(|c| {
    !(c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-'))
  });
  if let Some(c) = bad {
    bail!(
      "invalid {kind} {name:?}: must match [A-Za-z0-9 ._-]+, but contains {c:?}",
    );
  }
  Ok(())
}

/// The pieces of `dylib_path` we feed into the bundlers, with proper
/// error messages instead of `unwrap` panics on degenerate inputs like
/// `--output /` or `--output .`.
struct DylibParts<'a> {
  parent: &'a Path,
  file_name: &'a std::ffi::OsStr,
  app_name: String,
}

fn dylib_parts(dylib_path: &Path) -> Result<DylibParts<'_>, AnyError> {
  let parent = dylib_path.parent().ok_or_else(|| {
    deno_core::anyhow::anyhow!(
      "invalid --output: dylib path has no parent dir: {}",
      dylib_path.display()
    )
  })?;
  let file_name = dylib_path.file_name().ok_or_else(|| {
    deno_core::anyhow::anyhow!(
      "invalid --output: dylib path has no file name: {}",
      dylib_path.display()
    )
  })?;
  let app_name = dylib_path
    .file_stem()
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "invalid --output: dylib path has no file stem: {}",
        dylib_path.display()
      )
    })?
    .to_string_lossy()
    .into_owned();
  Ok(DylibParts {
    parent,
    file_name,
    app_name,
  })
}

/// Create a macOS .app bundle from the compiled desktop dylib.
///
/// Bundle structure:
/// ```text
/// AppName.app/
///   Contents/
///     Info.plist
///     MacOS/
///       AppName          (launcher script)
///       wef_webview      (WEF backend binary)
///       libapp.dylib     (compiled Deno runtime + user code)
///     Resources/
///       AppIcon.icns     (optional)
/// ```
async fn package_macos_app_bundle(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  wef_resolver: &WefBackendResolver,
) -> Result<PathBuf, AnyError> {
  let parts = dylib_parts(dylib_path)?;
  let app_name = parts.app_name.clone();
  let app_bundle = parts.parent.join(format!("{}.app", app_name));

  // Find the WEF backend .app and its main executable.
  let backend = desktop_flags.backend.as_deref().unwrap_or("cef");
  let target = wef_target_for(desktop_flags);
  let wef_app = wef_resolver.find_app_bundle(backend, target).await?;
  let wef_executable_name = read_plist_string(
    &wef_app.join("Contents/Info.plist"),
    "CFBundleExecutable",
  )
  .unwrap_or_else(|| "wef_webview".to_string());
  let wef_binary = wef_app.join("Contents/MacOS").join(&wef_executable_name);
  if !wef_binary.exists() {
    bail!(
      "WEF backend executable not found at '{}'",
      wef_binary.display()
    );
  }

  // Remove existing bundle.
  if app_bundle.exists() {
    std::fs::remove_dir_all(&app_bundle)?;
  }

  // Copy the entire WEF .app as the shell (CEF needs Frameworks/, Resources/, etc.).
  crate::tools::compile::copy_dir_all(&wef_app, &app_bundle)?;

  let contents_dir = app_bundle.join("Contents");
  let macos_dir = contents_dir.join("MacOS");
  let resources_dir = contents_dir.join("Resources");
  std::fs::create_dir_all(&resources_dir)?;

  // The backend binary extracts its self-extracting VFS to a sibling
  // `.<exe>` dir on first run. If the source wef.app was ever run, that dir
  // gets copied along with it — drop any such runtime caches.
  let wef_exe_stem = Path::new(&wef_executable_name)
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| wef_executable_name.clone());
  let cache_dir = macos_dir.join(format!(".{}", wef_exe_stem));
  if cache_dir.exists() {
    let _ = std::fs::remove_dir_all(&cache_dir);
  }
  let cache_file = macos_dir.join(format!(".{}.cache", wef_exe_stem));
  if cache_file.exists() {
    let _ = std::fs::remove_file(&cache_file);
  }

  // Strip unnecessary bulk from the CEF framework.
  strip_cef_bloat(&contents_dir);

  // Copy the compiled dylib.
  let dylib_filename = parts.file_name;
  let dest_dylib = macos_dir.join(dylib_filename);
  std::fs::copy(dylib_path, &dest_dylib)?;

  // Create launcher script as the main executable. Validate every name
  // we interpolate: bash expands `$VAR`, backticks, and `$(...)` even
  // inside `"..."`.
  let dylib_filename_str = dylib_filename.to_string_lossy();
  validate_launcher_name(&app_name, "app name")?;
  validate_launcher_name(&wef_executable_name, "WEF backend executable name")?;
  validate_launcher_name(&dylib_filename_str, "dylib filename")?;
  let launcher_path = macos_dir.join(&app_name);
  std::fs::write(
    &launcher_path,
    format!(
      "#!/bin/sh\n\
       DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
       exec \"$DIR/{wef_binary}\" --runtime \"$DIR/{dylib}\" \"$@\"\n",
      wef_binary = wef_executable_name,
      dylib = dylib_filename_str,
    ),
  )?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
      &launcher_path,
      std::fs::Permissions::from_mode(0o755),
    )?;
  }

  // Generate Info.plist.
  let has_icon = desktop_flags.icon.is_some();
  let bundle_id = app_name.to_lowercase().replace(' ', "-");
  let info_plist = format!(
    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>{app_name}</string>
  <key>CFBundleIconFile</key>
  <string>{icon_file}</string>
  <key>CFBundleIdentifier</key>
  <string>com.deno.desktop.{bundle_id}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>{app_name}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundleVersion</key>
  <string>1.0.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>10.15</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key>
  <true/>
  <key>NSAppTransportSecurity</key>
  <dict>
    <key>NSAllowsLocalNetworking</key>
    <true/>
  </dict>
</dict>
</plist>
"#,
    app_name = app_name,
    bundle_id = bundle_id,
    icon_file = if has_icon { "AppIcon" } else { "" },
  );
  std::fs::write(contents_dir.join("Info.plist"), info_plist)?;

  // Handle icon.
  if let Some(ref icon) = desktop_flags.icon {
    let dest = resources_dir.join("AppIcon.icns");
    match icon {
      crate::args::IconConfig::Single(path) => {
        let icon_path = cli_options.initial_cwd().join(path);
        if icon_path.exists() {
          match icon_path.extension().and_then(|e| e.to_str()) {
            Some("icns") => {
              std::fs::copy(&icon_path, &dest)?;
            }
            Some("png") => {
              crate::tools::compile::convert_png_to_icns(&icon_path, &dest)?;
            }
            _ => {
              log::warn!(
                "Icon '{}' is not .icns or .png, skipping",
                icon_path.display()
              );
            }
          }
        } else {
          log::warn!("Icon '{}' not found, skipping", icon_path.display());
        }
      }
      crate::args::IconConfig::Set(entries) => {
        convert_icon_set_to_icns(cli_options.initial_cwd(), entries, &dest)?;
      }
    }
  }

  // Remove the standalone dylib (it's now inside the .app).
  let _ = std::fs::remove_file(dylib_path);

  Ok(app_bundle)
}

/// Wrap a macOS `.app` bundle in a drag-to-Applications `.dmg` installer.
///
/// Builds a staging directory containing the `.app` plus a symlink to
/// `/Applications`, then invokes `hdiutil` to create a compressed read-only
/// disk image.
fn create_macos_dmg(
  app_bundle: &Path,
  dmg_path: &Path,
) -> Result<(), AnyError> {
  let app_name = app_bundle
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| "App".to_string());

  // Stage in a sibling temp directory so hdiutil doesn't traverse
  // unrelated files. tempfile gives us a unique name (no collision
  // with concurrent builds) and 0700 mode (no other-user racing or
  // pre-creating it as a symlink), and cleans up on drop.
  let parent = dmg_path
    .parent()
    .filter(|p| !p.as_os_str().is_empty())
    .unwrap_or_else(|| Path::new("."));
  std::fs::create_dir_all(parent)?;
  let staging = tempfile::Builder::new()
    .prefix(".dmg-staging-")
    .tempdir_in(parent)
    .with_context(|| {
      format!("failed to stage DMG tempdir in {}", parent.display())
    })?;

  // Copy the .app into the staging dir and add an /Applications symlink so
  // users can drag the app across in the mounted DMG window.
  let staged_app = staging.path().join(
    app_bundle
      .file_name()
      .ok_or_else(|| deno_core::anyhow::anyhow!("app bundle has no name"))?,
  );
  crate::tools::compile::copy_dir_all(app_bundle, &staged_app)?;
  #[cfg(unix)]
  {
    let _ = std::os::unix::fs::symlink(
      "/Applications",
      staging.path().join("Applications"),
    );
  }

  if dmg_path.exists() {
    std::fs::remove_file(dmg_path)?;
  }

  let status = std::process::Command::new("hdiutil")
    .args([
      "create",
      "-volname",
      &app_name,
      "-srcfolder",
      &staging.path().display().to_string(),
      "-ov",
      "-format",
      "UDZO",
      &dmg_path.display().to_string(),
    ])
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::inherit())
    .status()
    .context("Failed to run hdiutil")?;

  // staging tempdir is removed by its Drop impl when this fn returns.

  if !status.success() {
    bail!("hdiutil failed to create DMG at {}", dmg_path.display());
  }
  Ok(())
}

/// AppImage Type-2 runtime ELF stubs, vendored from
/// github.com/AppImage/type2-runtime at tag `20251108`. Prepended verbatim to
/// the SquashFS payload to form the final AppImage.
const APPIMAGE_RUNTIME_X86_64: &[u8] =
  include_bytes!("appimage_runtime/runtime-x86_64");
const APPIMAGE_RUNTIME_AARCH64: &[u8] =
  include_bytes!("appimage_runtime/runtime-aarch64");

/// 1×1 transparent PNG, used when the caller didn't supply an icon.
/// appimagetool-built AppImages expect a top-level `<Name>.png` to exist.
const STUB_ICON_PNG: &[u8] = &[
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49,
  0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
  0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44,
  0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d,
  0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42,
  0x60, 0x82,
];

/// Pick the Type-2 runtime stub for the requested target. Falls back to the
/// host arch when `target` is None. The triple's leading component is the
/// arch (e.g. `x86_64-unknown-linux-gnu` → `x86_64`).
fn appimage_runtime_for_target(
  target: Option<&str>,
) -> Result<&'static [u8], AnyError> {
  let arch = target
    .and_then(|t| t.split('-').next())
    .unwrap_or(std::env::consts::ARCH);
  match arch {
    "x86_64" => Ok(APPIMAGE_RUNTIME_X86_64),
    "aarch64" => Ok(APPIMAGE_RUNTIME_AARCH64),
    other => bail!(
      "No bundled AppImage runtime for arch '{other}'; supported: x86_64, aarch64"
    ),
  }
}

/// Unix mode bits for a filesystem entry. On non-Unix hosts (cross-compiling
/// a Linux AppImage from Windows/macOS) we don't have real mode bits, so fall
/// back to a reasonable default: 0o755 for dirs, 0o644 for files.
fn unix_mode_of(meta: &std::fs::Metadata) -> u16 {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    (meta.permissions().mode() & 0o7777) as u16
  }
  #[cfg(not(unix))]
  {
    if meta.is_dir() { 0o755 } else { 0o644 }
  }
}

fn node_header(mode: u16) -> backhand::NodeHeader {
  backhand::NodeHeader {
    permissions: mode,
    uid: 0,
    gid: 0,
    mtime: 0,
  }
}

/// Walk `fs_root` and push every entry into `writer` at the SquashFS root.
/// Directories are pushed before their contents (required by backhand).
fn push_dir_contents_to_squashfs(
  writer: &mut backhand::FilesystemWriter<'_, '_, '_>,
  fs_root: &Path,
) -> Result<(), AnyError> {
  let mut stack: Vec<PathBuf> = vec![fs_root.to_path_buf()];
  while let Some(dir) = stack.pop() {
    let mut entries: Vec<_> =
      std::fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
      let path = entry.path();
      let rel = path.strip_prefix(fs_root)?;
      let arc_path = Path::new("/").join(rel);
      let meta = std::fs::symlink_metadata(&path)?;
      let mode = unix_mode_of(&meta);
      let ft = meta.file_type();
      if ft.is_symlink() {
        let target = std::fs::read_link(&path)?;
        writer.push_symlink(target, &arc_path, node_header(mode))?;
      } else if ft.is_dir() {
        writer.push_dir(&arc_path, node_header(mode))?;
        stack.push(path);
      } else {
        let f = std::fs::File::open(&path)?;
        writer.push_file(f, &arc_path, node_header(mode))?;
      }
    }
  }
  Ok(())
}

/// Wrap a Linux app directory in an `.AppImage` single-file executable.
///
/// Packs the app dir into a SquashFS image via the `backhand` crate, adds the
/// AppDir-required entries (`AppRun`, `.desktop`, top-level icon), then
/// prepends the vendored AppImage Type-2 runtime ELF for the target arch.
/// Pure Rust; works on any build host.
fn create_linux_appimage(
  app_dir: &Path,
  appimage_path: &Path,
  target: Option<&str>,
) -> Result<(), AnyError> {
  use std::io::Cursor;
  use std::io::Write as _;

  let app_name = app_dir
    .file_name()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| "App".to_string());

  let runtime_elf = appimage_runtime_for_target(target)?;

  let mut writer = backhand::FilesystemWriter::default();

  // Pack everything from the staged app dir into the SquashFS root.
  push_dir_contents_to_squashfs(&mut writer, app_dir)?;

  // AppRun is what the AppImage invokes on launch. Thin shell shim that
  // delegates to the existing launcher (which already sets $DIR and execs
  // the backend with the right args).
  let apprun = format!(
    "#!/bin/sh\n\
     DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
     exec \"$DIR/{app_name}\" \"$@\"\n",
  );
  writer.push_file(
    Cursor::new(apprun.into_bytes()),
    "/AppRun",
    node_header(0o755),
  )?;

  // .desktop entry at the AppDir root.
  let desktop_entry = format!(
    "[Desktop Entry]\n\
     Type=Application\n\
     Name={app_name}\n\
     Exec={app_name}\n\
     Icon={app_name}\n\
     Categories=Utility;\n",
  );
  writer.push_file(
    Cursor::new(desktop_entry.into_bytes()),
    format!("/{app_name}.desktop"),
    node_header(0o644),
  )?;

  // Icon at AppDir root named after the app. package_linux_app_dir writes the
  // user icon as AppIcon.png; if absent, fall back to a 1×1 transparent PNG.
  let icon_src = app_dir.join("AppIcon.png");
  let icon_bytes = if icon_src.exists() {
    std::fs::read(&icon_src)?
  } else {
    STUB_ICON_PNG.to_vec()
  };
  writer.push_file(
    Cursor::new(icon_bytes),
    format!("/{app_name}.png"),
    node_header(0o644),
  )?;

  // Serialize the SquashFS to memory.
  let mut squashfs = Cursor::new(Vec::<u8>::new());
  writer
    .write(&mut squashfs)
    .context("Failed to write SquashFS image")?;
  drop(writer);

  // Assemble: runtime ELF + SquashFS, then mark executable.
  if let Some(parent) = appimage_path.parent()
    && !parent.as_os_str().is_empty()
  {
    std::fs::create_dir_all(parent)?;
  }
  let mut out = std::fs::File::create(appimage_path).with_context(|| {
    format!("Failed to create AppImage at {}", appimage_path.display())
  })?;
  out.write_all(runtime_elf)?;
  out.write_all(&squashfs.into_inner())?;
  drop(out);

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
      appimage_path,
      std::fs::Permissions::from_mode(0o755),
    )?;
  }

  Ok(())
}

/// Recursively copy a directory tree, ensuring writable permissions on the
/// destination. This is needed because source files from the Nix store are
/// read-only.
/// Strip unnecessary files from a CEF-based app bundle to reduce size.
///
/// Removes:
/// - Non-English locale packs (~47MB)
/// - SwiftShader software Vulkan renderer (~16MB, not needed on macOS with Metal)
/// - OpenGL ES emulation library (~6.5MB, not needed on macOS with Metal)
fn strip_cef_bloat(contents_dir: &Path) {
  let frameworks_dir = contents_dir.join("Frameworks");
  let cef_framework = frameworks_dir
    .join("Chromium Embedded Framework.framework")
    .join("Versions")
    .join("A");

  if !cef_framework.exists() {
    return;
  }

  let cef_resources = cef_framework.join("Resources");
  let cef_libraries = cef_framework.join("Libraries");

  // Remove non-English locale packs.
  if let Ok(entries) = std::fs::read_dir(&cef_resources) {
    for entry in entries.flatten() {
      let name = entry.file_name();
      let name = name.to_string_lossy();
      if name.ends_with(".lproj") && name != "en.lproj" {
        let _ = std::fs::remove_dir_all(entry.path());
      }
    }
  }

  // Remove SwiftShader (software Vulkan fallback, not needed on macOS with Metal).
  let _ = std::fs::remove_file(cef_libraries.join("libvk_swiftshader.dylib"));
  let _ = std::fs::remove_file(cef_libraries.join("vk_swiftshader_icd.json"));
}

/// Build a macOS `.icns` from an icon set (multiple PNGs at specified sizes).
///
/// Writes the ICNS container format directly (macOS 10.7+ accepts PNG bytes
/// as the payload for every modern OSType code, so no re-encoding needed).
/// Each pixel size maps to one or two OSType codes — the 1x and 2x slots
/// that share that pixel dimension:
///   16px  → icp4
///   32px  → ic11 (16×16@2x) + icp5 (32×32)
///   64px  → ic12 (32×32@2x)
///   128px → ic07
///   256px → ic13 (128×128@2x) + ic08 (256×256)
///   512px → ic14 (256×256@2x) + ic09 (512×512)
///   1024px→ ic10 (512×512@2x)
fn convert_icon_set_to_icns(
  cwd: &Path,
  entries: &[crate::args::IconSetEntry],
  icns_path: &Path,
) -> Result<(), deno_core::error::AnyError> {
  use std::io::Write;

  let mut icons: Vec<(&'static [u8; 4], Vec<u8>)> = Vec::new();
  for entry in entries {
    let src = cwd.join(&entry.path);
    if !src.exists() {
      log::warn!("Icon '{}' not found, skipping", src.display());
      continue;
    }
    let data = std::fs::read(&src)?;
    for code in icns_ostypes_for_size(entry.size) {
      icons.push((*code, data.clone()));
    }
  }

  if icons.is_empty() {
    deno_core::anyhow::bail!("No valid icon images found for .icns");
  }

  // File header (8 bytes) + for each icon: 4-byte OSType + 4-byte length + payload.
  let total_size: u32 = icons
    .iter()
    .map(|(_, data)| 8 + data.len() as u32)
    .sum::<u32>()
    + 8;

  let mut buf = Vec::with_capacity(total_size as usize);
  buf.write_all(b"icns")?;
  buf.write_all(&total_size.to_be_bytes())?;
  for (code, data) in &icons {
    buf.write_all(*code)?;
    let entry_len = 8 + data.len() as u32;
    buf.write_all(&entry_len.to_be_bytes())?;
    buf.write_all(data)?;
  }

  std::fs::write(icns_path, &buf)?;
  Ok(())
}

/// Returns the ICNS OSType codes a given pixel size maps to. Multiple codes
/// mean the same PNG is embedded under each (matching how the staged
/// `.iconset` approach duplicated files across 1x/2x filename slots).
fn icns_ostypes_for_size(size: u32) -> &'static [&'static [u8; 4]] {
  match size {
    16 => &[b"icp4"],
    32 => &[b"ic11", b"icp5"],
    64 => &[b"ic12"],
    128 => &[b"ic07"],
    256 => &[b"ic13", b"ic08"],
    512 => &[b"ic14", b"ic09"],
    1024 => &[b"ic10"],
    _ => {
      log::warn!(
        "Icon size {}px doesn't map to a standard macOS iconset slot, skipping",
        size
      );
      &[]
    }
  }
}

/// Build a Windows `.ico` from an icon set (multiple PNGs at specified sizes).
///
/// The ICO format stores PNG images directly (Vista+ supports PNG-compressed
/// entries). We write the ICO header, one directory entry per image, then the
/// raw PNG data for each.
pub fn convert_icon_set_to_ico(
  cwd: &Path,
  entries: &[crate::args::IconSetEntry],
  ico_path: &Path,
) -> Result<(), deno_core::error::AnyError> {
  use std::io::Write;

  let mut images: Vec<(u32, Vec<u8>)> = Vec::new();
  for entry in entries {
    let src = cwd.join(&entry.path);
    if !src.exists() {
      log::warn!("Icon '{}' not found, skipping", src.display());
      continue;
    }
    let data = std::fs::read(&src)?;
    images.push((entry.size, data));
  }

  if images.is_empty() {
    deno_core::anyhow::bail!("No valid icon images found for .ico");
  }

  let count = images.len() as u16;
  // ICO header: 6 bytes
  // Each directory entry: 16 bytes
  let header_size = 6 + (count as u32) * 16;

  let mut buf = Vec::new();
  // ICO header
  buf.write_all(&0u16.to_le_bytes())?; // reserved
  buf.write_all(&1u16.to_le_bytes())?; // type: 1 = ICO
  buf.write_all(&count.to_le_bytes())?; // image count

  // Directory entries
  let mut data_offset = header_size;
  for (size, data) in &images {
    // Width/height: 0 means 256 in ICO format
    let dim = if *size >= 256 { 0u8 } else { *size as u8 };
    buf.push(dim); // width
    buf.push(dim); // height
    buf.push(0); // color palette count
    buf.push(0); // reserved
    buf.write_all(&1u16.to_le_bytes())?; // color planes
    buf.write_all(&32u16.to_le_bytes())?; // bits per pixel
    buf.write_all(&(data.len() as u32).to_le_bytes())?; // image data size
    buf.write_all(&data_offset.to_le_bytes())?; // offset to image data
    data_offset += data.len() as u32;
  }

  // Image data
  for (_, data) in &images {
    buf.write_all(data)?;
  }

  std::fs::write(ico_path, &buf)?;
  Ok(())
}
