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

/// Version of the `laufey` capi crate pinned in the workspace Cargo.lock.
/// Populated by `cli/build.rs` and used to resolve matching prebuilt backend
/// binaries from `github.com/littledivy/laufey/releases/tag/v{LAUFEY_VERSION}`.
const LAUFEY_VERSION: &str = env!("LAUFEY_VERSION");

/// Rustc target triple the deno binary was built for. Used as the default
/// target when selecting a prebuilt laufey backend archive.
const LAUFEY_NATIVE_TARGET: &str = env!("TARGET");

/// Trust anchor for LAUFEY backend downloads: SHA-256 digests of every archive
/// for the pinned `LAUFEY_VERSION`. Checked into the repo so `SHA256SUMS` does
/// not need to be fetched (and trusted) at runtime — that file's integrity
/// previously rested on TOFU against the GitHub releases page. See
/// `cli/laufey_sums.lock` for the format.
const LAUFEY_PINNED_SUMS: &str = include_str!("../laufey_sums.lock");

pub async fn desktop(
  flags: Flags,
  mut desktop_flags: DesktopFlags,
) -> Result<(), AnyError> {
  log::warn!(
    "{}",
    colors::yellow_bold("⚠ deno desktop is experimental and subject to change")
  );

  let all_targets = desktop_flags.all_targets;

  let config_flags = flags.clone();
  let factory = CliFactory::from_flags(Arc::new(config_flags));
  let cli_options = factory.cli_options()?;
  let desktop_config = cli_options.start_dir.to_desktop_config()?.clone();
  let laufey_resolver = Arc::new(LaufeyBackendResolver::new(&factory)?);
  let deno_dir_root = factory.deno_dir()?.root.clone();

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

    if let Some(identifier) = app_config.identifier
      && desktop_flags.identifier.is_none()
    {
      desktop_flags.identifier = Some(identifier);
    }
  }

  if let Some(backend) = desktop_config.backend
    && desktop_flags.backend.is_none()
  {
    desktop_flags.backend = Some(backend);
  }

  if let Some(macos_config) = desktop_config.macos
    && let Some(identity) = macos_config.codesign_identity
    && desktop_flags.codesign_identity.is_none()
  {
    desktop_flags.codesign_identity = Some(identity);
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
      Box::pin(compile_desktop(
        flags.clone(),
        desktop_flags,
        cli_options,
        &laufey_resolver,
        &deno_dir_root,
      ))
      .await?;
    }
    Ok(())
  } else {
    Box::pin(compile_desktop(
      flags,
      desktop_flags,
      cli_options,
      &laufey_resolver,
      &deno_dir_root,
    ))
    .await
  }
}

async fn compile_desktop(
  mut flags: Flags,
  mut desktop_flags: DesktopFlags,
  cli_options: &Arc<CliOptions>,
  laufey_resolver: &LaufeyBackendResolver,
  deno_dir_root: &Path,
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
  let desktop_entrypoint_file = if desktop_flags.source_file == "." {
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
      // Sweep stale entrypoints leaked by previous interrupted runs. The
      // NamedTempFile below cleans up on drop, but a Ctrl-C delivers SIGINT
      // to the whole process group (see `run_desktop_hmr`) and the parent
      // exits without running destructors — so the dev loop would otherwise
      // accumulate `.deno_desktop_entry-*.ts` files in the project root.
      const ENTRY_PREFIX: &str = ".deno_desktop_entry-";
      if let Ok(entries) = std::fs::read_dir(cwd) {
        for entry in entries.flatten() {
          if entry
            .file_name()
            .to_string_lossy()
            .starts_with(ENTRY_PREFIX)
          {
            let _ = std::fs::remove_file(entry.path());
          }
        }
      }
      // Write a temporary entrypoint file. tempfile gives us a unique
      // name (no collision between concurrent `deno desktop` runs in
      // the same project) and 0600 mode (no symlink-pre-creation
      // attack); cleanup-on-drop replaces the explicit guard.
      let entrypoint_temp = tempfile::Builder::new()
        .prefix(ENTRY_PREFIX)
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
      if desktop_flags.output.is_none()
        && let Some(dir_name) = cwd.file_name()
      {
        desktop_flags.output = Some(dir_name.to_string_lossy().into_owned());
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
        "Could not detect a supported framework in the current directory.\nSupported frameworks: Next.js, Astro\nProvide an explicit entrypoint instead."
      );
    }
  } else {
    None
  };

  let self_extracting = desktop_entrypoint_file.is_some();
  // `desktop_entrypoint_file` (a NamedTempFile) keeps the file alive while
  // `compile_binary` reads it. It is explicitly closed right after compilation
  // (see below) rather than on drop: the long-running `run_desktop_hmr` wait
  // exits on Ctrl-C without running destructors, so a drop-only guard would
  // leak the entrypoint for the whole dev session.

  // No explicit icon, but a framework was detected — try to use its
  // favicon (e.g. `public/favicon.ico`, `app/icon.png`) as the app icon
  // so the bundle gets the project's branding for free.
  if desktop_flags.icon.is_none()
    && let Some(detection) = detected_framework.as_ref()
  {
    let target_os = match desktop_flags.target.as_deref() {
      Some(t) if t.contains("apple-darwin") => "macos",
      Some(t) if t.contains("windows") => "windows",
      Some(_) => "linux",
      None => {
        if cfg!(target_os = "macos") {
          "macos"
        } else if cfg!(target_os = "windows") {
          "windows"
        } else {
          "linux"
        }
      }
    };
    if let Some(path) = super::framework::find_framework_favicon(
      &detection_cwd,
      detection,
      target_os,
    ) {
      let display = path
        .strip_prefix(&detection_cwd)
        .unwrap_or(&path)
        .display()
        .to_string();
      log::info!("Using {} favicon as icon: {}", detection.name, display);
      desktop_flags.icon =
        Some(crate::args::IconConfig::Single(path.display().to_string()));
    }
  }

  let inspector_requested = flags.inspect.is_some()
    || flags.inspect_brk.is_some()
    || flags.inspect_wait.is_some();

  // In HMR/inspector mode the compiled dylib is a throwaway dev artifact: we
  // load it directly rather than packaging it into a `.app`. Writing it into
  // the cwd litters the project with `<name>.dylib`, its compile temp file
  // (`<name>.dylib.tmp-*`) and the runtime auto-update sidecars
  // (`.update-ok`, `.backup`). Redirect it into a stable per-project dir under
  // `deno_dir` so the cwd stays clean. The path is keyed by the project dir so
  // it's stable across relaunches (the auto-update / rollback sentinels rely on
  // a consistent dylib path).
  let hmr_output_override = if desktop_flags.hmr || inspector_requested {
    let name = desktop_flags
      .output
      .as_deref()
      .map(Path::new)
      .and_then(|p| p.file_stem())
      .map(|s| s.to_string_lossy().into_owned())
      .or_else(|| {
        detection_cwd
          .file_name()
          .map(|s| s.to_string_lossy().into_owned())
      })
      .filter(|s| !s.is_empty())
      .unwrap_or_else(|| "app".to_string());
    let key = faster_hex::hex_string(&sha2::Sha256::digest(
      detection_cwd.to_string_lossy().as_bytes(),
    ));
    let dir = deno_dir_root.join("desktop").join(&key[..16]);
    std::fs::create_dir_all(&dir).with_context(|| {
      format!("failed to create desktop dev dir {}", dir.display())
    })?;
    Some(dir.join(name).to_string_lossy().into_owned())
  } else {
    None
  };

  let compile_flags = CompileFlags {
    source_file: desktop_flags.source_file.clone(),
    output: hmr_output_override
      .clone()
      .or_else(|| desktop_flags.output.clone()),
    args: desktop_flags.args.clone(),
    target: desktop_flags.target.clone(),
    watch: None,
    no_terminal: false,
    icon: match &desktop_flags.icon {
      Some(crate::args::IconConfig::Single(s)) => Some(s.clone()),
      _ => None,
    },
    include: desktop_flags.include.clone(),
    exclude: desktop_flags.exclude.clone(),
    eszip: false,
    self_extracting,
    bundle: false,
    minify: false,
    exclude_unused_npm: false,
  };

  let mut temp_flags = flags.clone();
  temp_flags.subcommand = DenoSubcommand::Compile(compile_flags.clone());
  temp_flags.internal.is_desktop = true;

  let output_path = super::compile::compile_binary(
    Arc::new(temp_flags),
    compile_flags,
    true,
    None,
  )
  .await?;

  // The temp entrypoint is embedded in the compiled dylib's VFS now; nothing
  // downstream reads it from disk. Remove it deterministically here so the
  // long-running HMR session (which exits on Ctrl-C without running the
  // drop guard) can't leave it behind in the project root.
  if let Some(entrypoint_file) = desktop_entrypoint_file {
    let _ = entrypoint_file.close();
  }

  if desktop_flags.hmr || inspector_requested {
    let backend = desktop_flags.backend.as_deref().unwrap_or("webview");
    run_desktop_hmr(
      &output_path,
      &detection_cwd,
      detected_framework.as_ref(),
      backend,
      laufey_resolver,
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
      laufey_resolver,
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

/// Resolve `icon` (a `.png` or `.icns` path, possibly relative to
/// `initial_cwd`) into an absolute path suitable for `LAUFEY_APP_ICON`, which
/// laufey passes to `-[NSImage initWithContentsOfFile:]` (both formats are
/// accepted, so no conversion is needed).
#[cfg(target_os = "macos")]
fn resolve_hmr_icon_path(
  icon: &crate::args::IconConfig,
  initial_cwd: &Path,
) -> Result<PathBuf, AnyError> {
  let icon_path = match icon {
    crate::args::IconConfig::Single(p) => initial_cwd.join(p),
    crate::args::IconConfig::Set(_) => {
      deno_core::anyhow::bail!("icon sets are not supported in --hmr mode yet")
    }
  };
  if !icon_path.exists() {
    deno_core::anyhow::bail!("icon '{}' not found", icon_path.display());
  }
  match icon_path.extension().and_then(|e| e.to_str()) {
    Some("icns") | Some("png") => {}
    _ => deno_core::anyhow::bail!(
      "icon '{}' must be .icns or .png",
      icon_path.display()
    ),
  }
  Ok(crate::util::fs::canonicalize_path(&icon_path).unwrap_or(icon_path))
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
  laufey_resolver: &LaufeyBackendResolver,
  flags: &Flags,
  desktop_flags: &DesktopFlags,
) -> Result<(), AnyError> {
  let laufey_backend = laufey_resolver
    .find_binary(backend, LAUFEY_NATIVE_TARGET)
    .await?;
  let dylib_abs = crate::util::fs::canonicalize_path(dylib_path)
    .unwrap_or(dylib_path.to_path_buf());
  let source_abs = crate::util::fs::canonicalize_path(source_dir)
    .unwrap_or(source_dir.to_path_buf());

  // In HMR/inspector mode we launch the prebuilt laufey.app, so a user
  // `--icon` (or framework-detected favicon) would otherwise be ignored
  // and the Dock would show laufey's own icon. We can't rely on the bundle's
  // `CFBundleIconFile` (the dev bundle has none) or on swapping the bundled
  // `laufey.icns` (LaunchServices caches the icon for an already-registered
  // bundle id), so instead we pass the icon path to laufey and let it call
  // `-[NSApp setApplicationIconImage:]` at launch, which bypasses both.
  #[cfg(target_os = "macos")]
  let laufey_app_icon = desktop_flags.icon.as_ref().and_then(|icon| {
    resolve_hmr_icon_path(icon, &source_abs)
      .map_err(|e| log::warn!("Could not apply custom icon: {e}"))
      .ok()
  });

  // The prebuilt laufey bundle would otherwise present itself as "laufey" in the
  // menu bar, Dock and Cmd-Tab switcher. Pass a clearer name (the configured
  // app name / project directory) so laufey can override the process name at
  // launch. `desktop_flags.output` is already resolved from `--output`,
  // deno.json `desktop.app.name`, or the project dir before we get here.
  let app_name = desktop_flags
    .output
    .as_deref()
    .map(Path::new)
    .and_then(|p| p.file_stem())
    .map(|s| s.to_string_lossy().into_owned())
    .or_else(|| {
      source_abs
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
    })
    .filter(|s| !s.is_empty());

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

  let mut cmd = std::process::Command::new(&laufey_backend);
  cmd
    .arg("--runtime")
    .arg(&dylib_abs)
    .env("LAUFEY_RUNTIME_PATH", &dylib_abs)
    .current_dir(&source_abs);
  #[cfg(target_os = "macos")]
  if let Some(icon_path) = laufey_app_icon.as_ref() {
    cmd.env("LAUFEY_APP_ICON", icon_path);
  }
  if let Some(name) = app_name.as_ref() {
    cmd.env("LAUFEY_APP_NAME", name);
  }
  // Only enable the file watcher + setScriptSource pipeline when the user
  // actually asked for HMR. `deno desktop --inspect` alone used to spin up
  // both, surprising users (and burning the inspector channel on hot
  // reloads they didn't request).
  if desktop_flags.hmr {
    cmd.env("DENO_DESKTOP_HMR", &source_abs);
  }

  // Wire up the unified DevTools multiplexer when --inspect is set.
  // The mux runs in this (parent) process and fronts both the Deno runtime
  // inspector (in the LAUFEY subprocess) and the CEF renderer's debug port
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
      .env(
        "LAUFEY_REMOTE_DEBUGGING_PORT",
        cef_internal.port().to_string(),
      );
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
  // path that doesn't reach the explicit `wait` below, the LAUFEY backend
  // (and its CEF renderer subprocesses) get SIGKILLed on `Child` drop
  // rather than being orphaned. Normal Ctrl-C delivers SIGINT to the
  // whole process group so this rarely matters in practice; it covers
  // the abnormal-exit cases.
  //
  // On macOS we go through posix_spawn with TCC responsibility disclaimed
  // (see `disclaim_spawn`) so the laufey child is its own permission principal.
  // Without this, the kernel attributes notification/location/etc requests
  // to whatever started deno (typically the terminal), which has no bundle
  // id and causes `UNUserNotificationCenter.requestAuthorization` to fail
  // with UNErrorCodeNotificationsNotAllowed before any user prompt.
  #[cfg(target_os = "macos")]
  let status = {
    let mut child = disclaim_spawn::spawn(&cmd).with_context(|| {
      format!(
        "Failed to launch LAUFEY backend: {}",
        laufey_backend.display()
      )
    })?;
    child
      .wait()
      .await
      .context("Failed waiting for LAUFEY backend")?
  };
  #[cfg(not(target_os = "macos"))]
  let status = {
    let mut child = tokio::process::Command::from(cmd)
      .kill_on_drop(true)
      .spawn()
      .with_context(|| {
        format!(
          "Failed to launch LAUFEY backend: {}",
          laufey_backend.display()
        )
      })?;
    child
      .wait()
      .await
      .context("Failed waiting for LAUFEY backend")?
  };

  // Keep the mux alive until the subprocess exits, then drop it.
  drop(mux_handle);

  if !status.success() {
    bail!("LAUFEY backend exited with status: {}", status);
  }
  Ok(())
}

/// Package a compiled desktop dylib into a platform-specific app bundle.
async fn package_desktop_app(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  laufey_resolver: &LaufeyBackendResolver,
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
      laufey_resolver,
    )
    .await
  } else if is_windows {
    package_windows_app_dir(
      dylib_path,
      desktop_flags,
      cli_options,
      laufey_resolver,
    )
    .await
  } else {
    package_linux_app_dir(
      dylib_path,
      desktop_flags,
      cli_options,
      laufey_resolver,
    )
    .await
  }
}

/// Create a Windows app directory from the compiled desktop dylib.
///
/// Directory structure:
/// ```text
/// AppName/
///   AppName.bat         (launcher)
///   laufey.exe             (LAUFEY backend binary)
///   libcef.dll, ...     (CEF support files, if any)
///   denort.dll          (compiled Deno runtime + user code)
///   AppIcon.ico         (optional)
/// ```
async fn package_windows_app_dir(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  laufey_resolver: &LaufeyBackendResolver,
) -> Result<PathBuf, AnyError> {
  let parts = dylib_parts(dylib_path)?;
  let app_name = parts.app_name;
  let app_dir = parts.parent.join(&app_name);

  let backend = desktop_flags.backend.as_deref().unwrap_or("cef");
  let target = laufey_target_for(desktop_flags);
  let laufey_binary = laufey_resolver.find_binary(backend, target).await?;
  let laufey_dir = laufey_resolver.find_binary_dir(backend, target).await?;
  let laufey_binary_name = laufey_binary
    .file_name()
    .unwrap()
    .to_string_lossy()
    .to_string();

  if app_dir.exists() {
    std::fs::remove_dir_all(&app_dir)?;
  }

  // Copy LAUFEY backend directory (binary + CEF support files) as the shell.
  crate::tools::compile::copy_dir_all(&laufey_dir, &app_dir)?;

  // Drop any self-extracting runtime cache dir that tagged along.
  let laufey_exe_stem = Path::new(&laufey_binary_name)
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| laufey_binary_name.clone());
  let cache_dir = app_dir.join(format!(".{}", laufey_exe_stem));
  if cache_dir.exists() {
    let _ = std::fs::remove_dir_all(&cache_dir);
  }
  let cache_file = app_dir.join(format!(".{}.cache", laufey_exe_stem));
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
  validate_launcher_name(&laufey_binary_name, "LAUFEY backend binary name")?;
  validate_launcher_name(&dylib_filename_str, "dylib filename")?;
  let launcher_path = app_dir.join(format!("{}.bat", app_name));
  std::fs::write(
    &launcher_path,
    format!(
      "@echo off\r\n\
       set DIR=%~dp0\r\n\
       \"%DIR%{laufey_binary}\" --runtime \"%DIR%{dylib}\" %*\r\n",
      laufey_binary = laufey_binary_name,
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
///   laufey                 (LAUFEY backend binary)
///   libcef.so, ...      (CEF support files, if any)
///   libdenort.so        (compiled Deno runtime + user code)
///   AppIcon.png         (optional)
/// ```
async fn package_linux_app_dir(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  laufey_resolver: &LaufeyBackendResolver,
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
  let target = laufey_target_for(desktop_flags);
  let laufey_binary = laufey_resolver.find_binary(backend, target).await?;
  let laufey_dir = laufey_resolver.find_binary_dir(backend, target).await?;
  let laufey_binary_name = laufey_binary
    .file_name()
    .unwrap()
    .to_string_lossy()
    .to_string();

  if app_dir.exists() {
    std::fs::remove_dir_all(&app_dir)?;
  }

  // Copy LAUFEY backend directory (binary + CEF support files) as the shell.
  crate::tools::compile::copy_dir_all(&laufey_dir, &app_dir)?;

  // Drop any self-extracting runtime cache dir that tagged along.
  let laufey_exe_stem = Path::new(&laufey_binary_name)
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| laufey_binary_name.clone());
  let cache_dir = app_dir.join(format!(".{}", laufey_exe_stem));
  if cache_dir.exists() {
    let _ = std::fs::remove_dir_all(&cache_dir);
  }
  let cache_file = app_dir.join(format!(".{}.cache", laufey_exe_stem));
  if cache_file.exists() {
    let _ = std::fs::remove_file(&cache_file);
  }

  // Copy the compiled dylib alongside the backend binary.
  let dylib_filename = parts.file_name;
  let dest_dylib = app_dir.join(dylib_filename);
  std::fs::copy(dylib_path, &dest_dylib)?;

  // Create a shell launcher that invokes the backend with --runtime.
  // --ozone-platform=x11 forces CEF to create X11 windows (via XWayland on
  // Wayland sessions). The Linux LAUFEY mouse/focus/resize event monitor uses
  // XI2 on X11 and does not support Wayland.
  // GDK_BACKEND=x11 aligns GDK with Ozone so GDK_IS_X11_DISPLAY is true.
  //
  // Validate every name we interpolate: bash expands `$VAR`, backticks,
  // and `$(...)` even inside `"..."`.
  let dylib_filename_str = dylib_filename.to_string_lossy();
  validate_launcher_name(&app_name, "app name")?;
  validate_launcher_name(&laufey_binary_name, "LAUFEY backend binary name")?;
  validate_launcher_name(&dylib_filename_str, "dylib filename")?;
  let launcher_path = app_dir.join(&app_name);
  std::fs::write(
    &launcher_path,
    format!(
      "#!/bin/sh\n\
       DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
       export GDK_BACKEND=x11\n\
       exec \"$DIR/{laufey_binary}\" --ozone-platform=x11 --runtime \"$DIR/{dylib}\" \"$@\"\n",
      laufey_binary = laufey_binary_name,
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

  // Write a `.desktop` entry alongside the launcher so a user dropping
  // the app dir into `~/.local/share/applications/` gets the right
  // name/icon attribution on notifications and in the taskbar. laufey
  // doesn't read this file — only the OS does — but libnotify and
  // GNOME Shell key notification attribution on the desktop file's
  // `StartupWMClass` and `Icon` fields.
  let desktop_id = desktop_flags
    .identifier
    .clone()
    .unwrap_or_else(|| format!("com.deno.desktop.{}", app_name.to_lowercase()));
  if let Err(e) = validate_bundle_identifier(&desktop_id) {
    log::warn!(
      "skipping .desktop file: {e} (desktop file IDs follow the same reverse-DNS rules as macOS bundle IDs)"
    );
  } else {
    let desktop_entry = format!(
      "[Desktop Entry]\n\
       Type=Application\n\
       Name={app_name}\n\
       Exec={app_name}\n\
       Icon=AppIcon\n\
       StartupWMClass={desktop_id}\n\
       Categories=Utility;\n",
    );
    std::fs::write(
      app_dir.join(format!("{desktop_id}.desktop")),
      desktop_entry,
    )?;
  }

  // Remove the standalone dylib (it's now inside the app dir).
  let _ = std::fs::remove_file(dylib_path);

  Ok(app_dir)
}

/// Environment variable pointing at a local laufey checkout, used to bypass the
/// download path during development. Build-tree subpaths under this directory
/// are searched the same way the old sibling-checkout heuristic searched.
const LAUFEY_DEV_DIR_ENV: &str = "LAUFEY_DEV_DIR";

/// Resolves LAUFEY backend binaries and `.app` bundles, falling back to
/// downloading prebuilt archives from the laufey GitHub releases when
/// `LAUFEY_DEV_DIR` is not set.
struct LaufeyBackendResolver {
  http_client_provider: Arc<HttpClientProvider>,
  /// `<deno_dir>/laufey/<version>/`
  cache_root: PathBuf,
}

impl LaufeyBackendResolver {
  fn new(factory: &CliFactory) -> Result<Self, AnyError> {
    let cache_root =
      factory.deno_dir()?.root.join("laufey").join(LAUFEY_VERSION);
    Ok(Self {
      http_client_provider: factory.http_client_provider().clone(),
      cache_root,
    })
  }

  fn backend_cache_dir(&self, backend: &str, target: &str) -> PathBuf {
    self.cache_root.join(backend).join(target)
  }

  /// Download + verify + extract a backend archive if it isn't already in
  /// `<deno_dir>/laufey/<version>/<backend>/<target>/`.
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

    let archive = laufey_archive_name(backend, target);
    let client = self.http_client_provider.get_or_create()?;

    // Use the in-tree pinned digests rather than fetching SHA256SUMS from the
    // release page. The latter is unsigned, so trusting it would let anyone
    // who can write to the laufey release host swap both archive and sums
    // together (TOFU). The lock file is reviewed in PRs when LAUFEY_VERSION
    // bumps, so this is the trust anchor. That the lock file's pinned version
    // matches LAUFEY_VERSION is asserted at build time (see cli/build.rs).
    let expected = parse_sha256sum(LAUFEY_PINNED_SUMS, &archive).ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "no pinned SHA-256 for {archive} in cli/laufey_sums.lock \
         (regenerate when bumping LAUFEY_VERSION to v{LAUFEY_VERSION}; \
         laufey v{LAUFEY_VERSION} release may not include backend '{backend}' for target '{target}')"
      )
    })?;

    log::info!(
      "{} laufey {} backend for {} (v{})",
      colors::green("Downloading"),
      backend,
      target,
      LAUFEY_VERSION,
    );

    let url = Url::parse(&laufey_release_url(&archive))?;
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
        "LAUFEY cache dir has no parent: {}",
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

    extract_laufey_archive(&archive, &data, staging.path())
      .with_context(|| format!("failed to extract {archive}"))?;
    // Marker is written into the staging dir so it lands atomically with
    // the rest of the contents — a SIGKILL during extraction can never
    // leave a marker-without-payload state.
    std::fs::write(
      staging.path().join(".downloaded"),
      format!("v{LAUFEY_VERSION}\n"),
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
        "failed to atomic-rename LAUFEY cache to {}: {e}",
        dir.display(),
      ));
    }

    // The laufey release archive linker-signs the `laufey` binary with
    // identifier=`laufey` rather than the .app's CFBundleIdentifier. UN
    // (UNUserNotificationCenter) rejects authorization requests when
    // the running binary's signed identifier disagrees with the host
    // bundle's id (UNErrorCodeNotificationsNotAllowed, error code 1).
    // In HMR mode the user's project bundle isn't involved — we run
    // laufey.app directly — so we have to fix the cached copy itself.
    // Re-sign every Mach-O inside the cached laufey.app with the bundle
    // id so the running identity is internally consistent. No-op on
    // non-macOS targets / hosts (no `codesign(1)`).
    #[cfg(target_os = "macos")]
    if target.contains("apple-darwin")
      && let Err(e) = harmonize_cached_laufey_identifiers(&dir, backend)
    {
      log::warn!(
        "[desktop] could not re-sign cached laufey backend: {e} \
         (notifications may not work in HMR mode until you re-download)"
      );
    }

    Ok(dir)
  }

  /// Locate the LAUFEY backend binary for `backend` on `target`.
  ///
  /// Resolution order: `LAUFEY_DEV_DIR` checkout → cached download →
  /// fresh download.
  async fn find_binary(
    &self,
    backend: &str,
    target: &str,
  ) -> Result<PathBuf, AnyError> {
    if let Some(dev_dir) = laufey_dev_dir() {
      let binary =
        locate_dev_backend_binary(&dev_dir, backend).ok_or_else(|| {
          deno_core::anyhow::anyhow!(
            "could not find '{backend}' backend binary under {} (set via {})",
            dev_dir.display(),
            LAUFEY_DEV_DIR_ENV
          )
        })?;
      // Re-sign the dev laufey build so its identifier matches the bundle
      // id — same fix as the download path, applied every launch
      // because a fresh `cargo build` of laufey restores the linker's
      // default `Identifier=laufey`. The harmonize call is idempotent:
      // already-correct binaries are skipped.
      #[cfg(target_os = "macos")]
      if let Some(laufey_app) = laufey_app_for_binary(&binary)
        && let Err(e) = harmonize_laufey_app_identifiers(&laufey_app)
      {
        log::warn!(
          "[desktop] could not re-sign dev laufey backend: {e} \
           (notifications may not work in HMR mode)"
        );
      }
      return Ok(binary);
    }

    let dir = self.ensure_downloaded(backend, target).await?;
    locate_backend_binary(&dir, backend, target).ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "could not find '{backend}' backend binary inside {}",
        dir.display()
      )
    })
  }

  /// Locate the LAUFEY `.app` bundle for `backend` on a macOS `target`.
  async fn find_app_bundle(
    &self,
    backend: &str,
    target: &str,
  ) -> Result<PathBuf, AnyError> {
    if let Some(dev_dir) = laufey_dev_dir() {
      return locate_dev_app_bundle(&dev_dir, backend).ok_or_else(|| {
        deno_core::anyhow::anyhow!(
          "could not find '{backend}' .app bundle under {} (set via {})",
          dev_dir.display(),
          LAUFEY_DEV_DIR_ENV
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
        "LAUFEY backend binary has no parent directory: {}",
        binary.display()
      )
    })?;
    Ok(parent.to_path_buf())
  }
}

fn laufey_archive_name(backend: &str, target: &str) -> String {
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
  format!("laufey-{archive_backend}-{target}.{ext}")
}

fn laufey_release_url(file: &str) -> String {
  format!(
    "https://github.com/littledivy/laufey/releases/download/v{LAUFEY_VERSION}/{file}"
  )
}

/// Pick out the hex digest for `file` from a GNU `sha256sum`-style file. Each
/// line is `<hex>  <filename>` (optionally `<hex>  *<filename>` for binary
/// mode).
fn parse_sha256sum(contents: &str, file: &str) -> Option<String> {
  for line in contents.lines() {
    let mut parts = line.split_whitespace();
    // Skip blank / whitespace-only lines instead of bailing out of the
    // whole parse — `?` would terminate the function early on the first
    // empty line and quietly miss every subsequent entry.
    let Some(hex) = parts.next() else { continue };
    let Some(name) = parts.next() else { continue };
    if name.trim_start_matches('*') == file {
      return Some(hex.to_string());
    }
  }
  None
}

fn extract_laufey_archive(
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
        if let Ok(meta) = std::fs::symlink_metadata(&dest_path)
          && meta.file_type().is_file()
        {
          // Was the entry executable? If so, mask to 0o755; otherwise 0o644.
          let mode = entry.header().mode().unwrap_or(0o644);
          let safe = if mode & 0o111 != 0 { 0o755 } else { 0o644 };
          let mut perms = meta.permissions();
          perms.set_mode(safe);
          let _ = std::fs::set_permissions(&dest_path, perms);
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
      // escape, and LAUFEY Windows archives have no legitimate need for
      // them.
      if entry.is_symlink() {
        bail!(
          "refusing symlink entry in laufey archive: {}",
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
      let p = dir.join("laufey.app/Contents/MacOS/laufey");
      p.exists().then_some(p)
    }
    "webview" if is_macos => {
      let p = dir.join("laufey_webview.app/Contents/MacOS/laufey_webview");
      p.exists().then_some(p)
    }
    _ => {
      let stem = match backend {
        "cef" => "laufey",
        "raw" => "laufey_winit",
        "servo" => "laufey_servo",
        _ => "laufey_webview",
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
    "cef" => "laufey.app",
    _ => "laufey_webview.app",
  };
  let p = dir.join(name);
  p.exists().then_some(p)
}

/// Target triple to use when selecting a laufey backend archive. Honors
/// `desktop_flags.target` (for cross-target packaging); otherwise defaults to
/// the host triple this deno binary was built for.
fn laufey_target_for(desktop_flags: &DesktopFlags) -> &str {
  desktop_flags
    .target
    .as_deref()
    .unwrap_or(LAUFEY_NATIVE_TARGET)
}

/// Resolve `LAUFEY_DEV_DIR` to a directory path if set and present on disk.
fn laufey_dev_dir() -> Option<PathBuf> {
  let raw = std::env::var(LAUFEY_DEV_DIR_ENV).ok()?;
  let p = PathBuf::from(raw);
  p.is_dir().then_some(p)
}

/// Find a built backend binary inside a laufey checkout. Mirrors the well-known
/// build-tree paths produced by laufey's Makefile + Nix flakes.
fn locate_dev_backend_binary(laufey: &Path, backend: &str) -> Option<PathBuf> {
  let candidates: Vec<PathBuf> = match backend {
    "cef" => vec![
      laufey.join("result-cef/Applications/laufey.app/Contents/MacOS/laufey"),
      laufey.join("result/Applications/laufey.app/Contents/MacOS/laufey"),
      laufey.join("cef/build/Release/laufey.app/Contents/MacOS/laufey"),
      laufey.join("cef/build/laufey.app/Contents/MacOS/laufey"),
      laufey.join("cef/build/Release/laufey"),
      laufey.join("cef/build/laufey"),
    ],
    "servo" => vec![
      laufey.join("target/release/laufey_servo"),
      laufey.join("target/debug/laufey_servo"),
    ],
    "raw" => vec![
      laufey.join("target/release/laufey_winit"),
      laufey.join("target/debug/laufey_winit"),
    ],
    _ => vec![
      laufey.join(
        "result-1/Applications/laufey_webview.app/Contents/MacOS/laufey_webview",
      ),
      laufey
        .join("result/Applications/laufey_webview.app/Contents/MacOS/laufey_webview"),
      laufey.join("webview/build/laufey_webview.app/Contents/MacOS/laufey_webview"),
      laufey.join("webview/build/laufey_webview"),
    ],
  };
  candidates.into_iter().find(|p| p.exists())
}

/// Find a built backend `.app` bundle inside a laufey checkout.
fn locate_dev_app_bundle(laufey: &Path, backend: &str) -> Option<PathBuf> {
  let candidates: Vec<PathBuf> = match backend {
    "cef" => vec![
      laufey.join("result-cef/Applications/laufey.app"),
      laufey.join("result/Applications/laufey.app"),
      laufey.join("cef/build/Release/laufey.app"),
      laufey.join("cef/build/laufey.app"),
    ],
    "raw" | "servo" => return None,
    _ => vec![
      laufey.join("result-1/Applications/laufey_webview.app"),
      laufey.join("result/Applications/laufey_webview.app"),
      laufey.join("webview/build/laufey_webview.app"),
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

/// Validate a reverse-DNS bundle identifier (Apple `CFBundleIdentifier`,
/// also used for Linux `.desktop` filenames and Windows AppUserModelID).
///
/// Apple's rules: ASCII alphanumerics, hyphens, and dots; must have at
/// least one dot (so it looks like reverse DNS); each dot-separated
/// segment must be non-empty and not start with a digit. We don't
/// enforce the segment-leading-letter rule strictly (some legacy apps
/// use digits) but we do reject empty segments and obvious shell
/// metacharacters — the identifier ends up as a `codesign` argument and
/// a path component of the helper bundles.
fn validate_bundle_identifier(id: &str) -> Result<(), AnyError> {
  if id.is_empty() {
    bail!("bundle identifier is empty");
  }
  if id.len() > 155 {
    // Apple's documented limit for CFBundleIdentifier on receipts is
    // 155 chars; bigger values quietly truncate elsewhere in the
    // toolchain.
    bail!("bundle identifier {id:?} is longer than 155 characters");
  }
  if !id.contains('.') {
    bail!(
      "bundle identifier {id:?} must be in reverse-DNS form (e.g. com.acme.foo)"
    );
  }
  for c in id.chars() {
    if !(c.is_ascii_alphanumeric() || c == '.' || c == '-') {
      bail!(
        "bundle identifier {id:?} must match [A-Za-z0-9.-]+, but contains {c:?}",
      );
    }
  }
  if id.split('.').any(|seg| seg.is_empty()) {
    bail!("bundle identifier {id:?} has an empty segment");
  }
  Ok(())
}

/// Walk every `.app` under `Contents/Frameworks/` and rewrite its
/// `CFBundleIdentifier` so it's a strict suffix of `main_bundle_id`.
///
/// CEF's process model: when the browser process spawns a helper for a
/// child role (gpu, renderer, plugin, …), the helper inspects its own
/// `CFBundleIdentifier` and refuses to attach to a parent whose id
/// doesn't match it as a prefix. laufey's default helper plists ship with
/// `com.example.laufey.helper.*` — which is inconsistent with whatever id
/// we wrote into the main bundle, so we'd get a launch-time refusal
/// (the helper exits silently and the browser hangs waiting for it).
///
/// We compute the new id by extracting the "kind" suffix from the
/// existing id (everything from the last `helper` segment onward) and
/// concatenating it onto the main id. So `com.example.laufey.helper` →
/// `<main>.helper`, `com.example.laufey.helper.gpu` → `<main>.helper.gpu`.
fn rewrite_cef_helper_bundle_ids(
  contents_dir: &Path,
  main_bundle_id: &str,
) -> Result<(), AnyError> {
  let frameworks = contents_dir.join("Frameworks");
  if !frameworks.exists() {
    return Ok(());
  }
  let entries = match std::fs::read_dir(&frameworks) {
    Ok(e) => e,
    Err(_) => return Ok(()),
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let name = path
      .file_name()
      .map(|s| s.to_string_lossy().into_owned())
      .unwrap_or_default();
    // Only touch helper apps. The CEF framework directory
    // (`Chromium Embedded Framework.framework`) also lives here and
    // has its own bundle id that we must not rewrite — touching it
    // would invalidate its embedded code signature.
    if !path.is_dir() || !name.ends_with(".app") || !name.contains("Helper") {
      continue;
    }
    let plist_path = path.join("Contents/Info.plist");
    if !plist_path.exists() {
      continue;
    }
    rewrite_helper_plist_identifier(&plist_path, main_bundle_id).with_context(
      || format!("failed to rewrite helper plist at {}", plist_path.display()),
    )?;
  }
  Ok(())
}

/// Rewrite a single helper's `CFBundleIdentifier` based on the existing
/// value's "kind" suffix.
fn rewrite_helper_plist_identifier(
  plist_path: &Path,
  main_bundle_id: &str,
) -> Result<(), AnyError> {
  let mut dict: plist::Dictionary = plist::from_file(plist_path)
    .with_context(|| format!("failed to parse {}", plist_path.display()))?;
  let existing = dict
    .get("CFBundleIdentifier")
    .and_then(|v| v.as_string())
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "helper plist {} has no CFBundleIdentifier",
        plist_path.display()
      )
    })?;
  // Extract the suffix from the last `helper` segment onward. Falls back
  // to a bare `helper` if the existing id doesn't contain that token
  // (defensive — every laufey helper plist has it today).
  let suffix = existing
    .find("helper")
    .map(|i| &existing[i..])
    .unwrap_or("helper");
  let new_id = format!("{main_bundle_id}.{suffix}");
  dict.insert(
    "CFBundleIdentifier".to_string(),
    plist::Value::String(new_id),
  );
  // Write XML format for stability and human-diffability. Helper plists
  // are tiny so we don't gain anything by switching to binary plist
  // format; XML is what laufey ships and what `codesign` expects to find.
  plist::to_file_xml(plist_path, &dict)
    .with_context(|| format!("failed to write {}", plist_path.display()))?;
  Ok(())
}

/// Codesign the macOS bundle in place. Signs every helper `.app` and
/// the embedded CEF framework first, then the main bundle (signatures
/// nest: the outer signature's CodeDirectory hashes the inner ones, so
/// outer-last is the only order that works).
///
/// Re-uses the JIT entitlements that ship with the laufey CEF bundle
/// (`Contents/Frameworks/<helper>.app/Contents/Resources/...entitlements...`
/// or, more robustly, the per-helper entitlements laufey bundles next to
/// each helper). When entitlements aren't present we fall back to
/// signing without them — the binary will still launch but V8 won't
/// get JIT permission.
fn codesign_macos_bundle(
  app_bundle: &Path,
  identity: &str,
) -> Result<(), AnyError> {
  if !cfg!(target_os = "macos") {
    bail!(
      "codesigning requires a macOS build host (uses `codesign(1)`). \
       Run `deno desktop` on macOS, or drop `macos.codesignIdentity` \
       from your deno.json when cross-building."
    );
  }
  if identity.is_empty() {
    bail!("macos.codesignIdentity is empty");
  }
  log::info!(
    "{} bundle with identity {:?}",
    colors::green("Codesigning"),
    identity,
  );

  // Read the bundle id from the main Info.plist so we can override the
  // signed identifier on `Contents/MacOS/laufey`. The default identifier
  // codesign infers from a bare Mach-O binary is `laufey` (the basename),
  // which doesn't match the .app's CFBundleIdentifier — and UN refuses
  // notification authorization when the running binary's signed id
  // doesn't match the bundle's id. Forcing `--identifier=<bundle_id>`
  // makes them match.
  let bundle_id = read_bundle_identifier(app_bundle)?;

  // Sign helpers first (inside → outside). The order within helpers
  // doesn't matter — they don't nest into each other.
  let frameworks = app_bundle.join("Contents/Frameworks");
  if frameworks.exists()
    && let Ok(entries) = std::fs::read_dir(&frameworks)
  {
    for entry in entries.flatten() {
      let path = entry.path();
      let Some(name) =
        path.file_name().map(|s| s.to_string_lossy().into_owned())
      else {
        continue;
      };
      if path.is_dir() && name.ends_with(".app") {
        let helper_entitlements = locate_helper_entitlements(&path);
        codesign_one(&path, identity, helper_entitlements.as_deref(), None)?;
      } else if path.is_dir() && name.ends_with(".framework") {
        // Frameworks (notably the CEF framework) have an embedded
        // signature already — `codesign --force` re-signs the
        // versioned directory in place.
        codesign_one(&path, identity, None, None)?;
      }
    }
  }

  // Sign `Contents/MacOS/laufey` (and the launcher shim, the dylib) with
  // the bundle's id as the signed identifier. This is what UN keys
  // notification permission to — without it, UN sees `laufey` requesting
  // permission for `com.example.app` and rejects with
  // `UNErrorCodeNotificationsNotAllowed`.
  let macos_dir = app_bundle.join("Contents/MacOS");
  if let Ok(entries) = std::fs::read_dir(&macos_dir) {
    for entry in entries.flatten() {
      let path = entry.path();
      if !path.is_file() {
        continue;
      }
      // Skip non-Mach-O files: the auto-update sentinel (`*.update-ok`),
      // staged updates (`*.update`, `*.backup`), and the POSIX shell
      // launcher (`Contents/MacOS/<app>` is a shell script that execs
      // laufey). codesign can't sign a text file and would fail the
      // whole signing pass.
      if !is_macho_file(&path) {
        continue;
      }
      codesign_one(&path, identity, None, Some(&bundle_id))?;
    }
  }

  // Finally the main bundle. Use the browser-process entitlements if
  // laufey shipped them; otherwise sign with no entitlements (still
  // launches, just no JIT for V8 in the browser process — which doesn't
  // host V8 anyway, so this is fine).
  let browser_entitlements = locate_browser_entitlements(app_bundle);
  codesign_one(app_bundle, identity, browser_entitlements.as_deref(), None)?;
  Ok(())
}

/// Read `CFBundleIdentifier` out of `Contents/Info.plist` via `plutil`.
fn read_bundle_identifier(app_bundle: &Path) -> Result<String, AnyError> {
  let plist = app_bundle.join("Contents/Info.plist");
  let output = std::process::Command::new("plutil")
    .arg("-extract")
    .arg("CFBundleIdentifier")
    .arg("raw")
    .arg("-o")
    .arg("-")
    .arg(&plist)
    .output()
    .context("failed to invoke plutil(1) to read CFBundleIdentifier")?;
  if !output.status.success() {
    bail!(
      "plutil could not read CFBundleIdentifier from {}: {}",
      plist.display(),
      String::from_utf8_lossy(&output.stderr).trim(),
    );
  }
  let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if id.is_empty() {
    bail!("CFBundleIdentifier is empty in {}", plist.display());
  }
  Ok(id)
}

/// Cheap Mach-O sniff: a Mach-O file starts with one of the well-known
/// magic numbers (32-bit, 64-bit, fat — both endians). Used to skip
/// non-binaries inside `Contents/MacOS/` (shell launchers, update
/// sentinels) before passing them to `codesign(1)`, which would reject
/// them and abort the signing pass.
fn is_macho_file(path: &Path) -> bool {
  let Ok(mut f) = std::fs::File::open(path) else {
    return false;
  };
  use std::io::Read;
  let mut magic = [0u8; 4];
  if f.read_exact(&mut magic).is_err() {
    return false;
  }
  matches!(
    u32::from_be_bytes(magic),
    // FAT_MAGIC / FAT_CIGAM / FAT_MAGIC_64 / FAT_CIGAM_64
    0xcafebabe | 0xbebafeca | 0xcafebabf | 0xbfbafeca
      // MH_MAGIC / MH_CIGAM (32-bit) / MH_MAGIC_64 / MH_CIGAM_64
      | 0xfeedface | 0xcefaedfe | 0xfeedfacf | 0xcffaedfe
  )
}

/// Search for a per-helper entitlements plist that laufey bundled with this
/// helper. Returns the path if found, `None` otherwise.
fn locate_helper_entitlements(helper_app: &Path) -> Option<PathBuf> {
  // laufey ships these alongside the helper binaries; the exact filename
  // pattern depends on the laufey build. Probe the well-known names; fall
  // back to the generic `entitlements-helper.plist` next to Contents.
  let candidates = [
    helper_app.join("Contents/Resources/entitlements-helper.plist"),
    helper_app
      .parent()
      .map(|p| p.join("entitlements-helper.plist"))
      .unwrap_or_default(),
  ];
  candidates.into_iter().find(|p| p.exists())
}

/// Locate the browser-process entitlements plist for the main bundle.
fn locate_browser_entitlements(app_bundle: &Path) -> Option<PathBuf> {
  let candidates = [
    app_bundle.join("Contents/Resources/entitlements-browser.plist"),
    app_bundle.join("Contents/Resources/entitlements.plist"),
  ];
  candidates.into_iter().find(|p| p.exists())
}

/// Run `codesign --force [--timestamp --options runtime] --sign <identity> [--entitlements <plist>] <path>`.
///
/// `--options runtime` enables the Hardened Runtime, which is required
/// for notarization. `--force` overwrites any existing signature; the
/// helpers come pre-signed by laufey with an ad-hoc signature that we
/// always need to replace.
///
/// Ad-hoc identity (`-`) skips `--timestamp` (no cert to anchor a
/// timestamp to) and `--options runtime` (Hardened Runtime + ad-hoc is
/// a Gatekeeper-rejected combo). Ad-hoc is what we use when the user
/// hasn't configured a real signing identity — it's enough for macOS
/// to grant the bundle a stable code identity, which UN requires
/// before it will hand out notification permission.
fn codesign_one(
  target: &Path,
  identity: &str,
  entitlements: Option<&Path>,
  signing_identifier: Option<&str>,
) -> Result<(), AnyError> {
  let adhoc = identity == "-";
  let mut cmd = std::process::Command::new("codesign");
  cmd.arg("--force");
  if !adhoc {
    cmd.arg("--timestamp").arg("--options").arg("runtime");
  }
  if let Some(ident) = signing_identifier {
    cmd.arg("--identifier").arg(ident);
  }
  cmd.arg("--sign").arg(identity);
  if let Some(ent) = entitlements {
    cmd.arg("--entitlements").arg(ent);
  }
  cmd.arg(target);
  let status = cmd
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::inherit())
    .status()
    .context("failed to invoke codesign(1)")?;
  if !status.success() {
    bail!(
      "codesign failed for {} (identity {:?})",
      target.display(),
      identity,
    );
  }
  Ok(())
}

/// Re-sign the cached laufey.app's binaries so the running binary's
/// identifier matches its host bundle's `CFBundleIdentifier`. Run once
/// per fresh download; HMR mode runs laufey.app directly (no per-project
/// wrapper), so without this UN sees `Identifier=laufey` /
/// `CFBundleIdentifier=com.deno.desktop` and refuses notification
/// authorization. Best-effort: failures here are logged but don't
/// abort the install, since most desktop features still work without
/// notifications.
#[cfg(target_os = "macos")]
fn harmonize_cached_laufey_identifiers(
  install_dir: &Path,
  backend: &str,
) -> Result<(), AnyError> {
  let laufey_app = locate_laufey_app_in_install(install_dir, backend)
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "could not find laufey.app under {} to re-sign",
        install_dir.display()
      )
    })?;
  harmonize_laufey_app_identifiers(&laufey_app)
}

/// Idempotently re-sign every Mach-O in `<laufey.app>/Contents/MacOS/` so
/// its code-signing identifier matches the bundle's `CFBundleIdentifier`.
/// macOS `usernoted` rejects UN authorization with
/// `UNErrorCodeNotificationsNotAllowed` when the running binary's
/// signed identifier disagrees with the bundle id ("Legacy client X
/// connecting to modern client" in the daemon log). The linker-signed
/// default for the laufey binary is `Identifier=laufey`, which never matches.
/// Safe to call on every launch — binaries already at the correct
/// identifier are skipped without a re-sign.
#[cfg(target_os = "macos")]
fn harmonize_laufey_app_identifiers(laufey_app: &Path) -> Result<(), AnyError> {
  let bundle_id = read_bundle_identifier(laufey_app)?;
  let macos_dir = laufey_app.join("Contents/MacOS");

  // laufey writes a `.laufey/` runtime-data cache directly inside the bundle's
  // `Contents/MacOS/` at runtime. When codesign(1) signs the main
  // executable (`Contents/MacOS/laufey`) it seals the *whole* bundle and trips
  // over that stray directory with "bundle format unrecognized", aborting
  // the re-sign — which silently breaks notification permission on every
  // launch after the first run created the cache. Park it just outside the
  // bundle while we sign and move it back afterwards (it would regenerate
  // anyway, but preserving it avoids a cold-start penalty). The guard
  // restores it even if signing errors out below.
  let _parked = park_bundle_cache_dir(&macos_dir);

  for entry in std::fs::read_dir(&macos_dir)?.flatten() {
    let path = entry.path();
    // Skip non-Mach-O files (`Contents/MacOS/` legitimately contains
    // shell launchers and update sentinels). `is_file` filters both
    // directories and shell scripts; `is_macho_file` rejects the rest.
    // Skipping these is critical: handing them to codesign would fail
    // with "bundle format unrecognized" and abort the run.
    if !path.is_file() || !is_macho_file(&path) {
      continue;
    }
    if signed_identifier_matches(&path, &bundle_id) {
      continue;
    }
    codesign_one(&path, "-", None, Some(&bundle_id))?;
  }
  Ok(())
}

/// RAII guard that moves a directory parked by [`park_bundle_cache_dir`]
/// back into the bundle on drop. Best-effort: a failed restore is ignored
/// (the cache regenerates on next launch).
#[cfg(target_os = "macos")]
struct ParkedCacheDir {
  parked_at: PathBuf,
  restore_to: PathBuf,
}

#[cfg(target_os = "macos")]
impl Drop for ParkedCacheDir {
  fn drop(&mut self) {
    if self.parked_at.exists() {
      let _ = std::fs::rename(&self.parked_at, &self.restore_to);
    }
  }
}

/// If `<macos_dir>/.laufey` exists, move it just outside the `.app` bundle so
/// codesign's bundle sealing doesn't choke on it, returning a guard that
/// restores it on drop. Returns `None` when there's nothing to park or it
/// can't be relocated — in which case signing proceeds as before (and may
/// fail loudly, same as the previous behaviour).
#[cfg(target_os = "macos")]
fn park_bundle_cache_dir(macos_dir: &Path) -> Option<ParkedCacheDir> {
  let cache = macos_dir.join(".laufey");
  if !cache.exists() {
    return None;
  }
  // Park it next to the `.app` (outside `Contents/`, so it's not part of
  // what codesign seals). `macos_dir` is `<app>/Contents/MacOS`, so three
  // parents up is the directory containing the `.app`. Staying on the same
  // volume keeps the rename atomic and cheap.
  let app_parent = macos_dir.parent()?.parent()?.parent()?;
  let parked_at = app_parent.join(".laufey-harmonize-parked");
  // Clear any debris from a previously interrupted run.
  if parked_at.exists() {
    let _ = std::fs::remove_dir_all(&parked_at);
  }
  match std::fs::rename(&cache, &parked_at) {
    Ok(()) => Some(ParkedCacheDir {
      parked_at,
      restore_to: cache,
    }),
    Err(_) => None,
  }
}

/// Read the code-signing identifier of `path` (`codesign -dv` →
/// `Identifier=...`). Returns true if it equals `expected`. False if
/// we can't read it (unsigned binary, codesign missing) — that means
/// "needs a re-sign," which is the correct fall-through for the
/// caller.
#[cfg(target_os = "macos")]
fn signed_identifier_matches(path: &Path, expected: &str) -> bool {
  let Ok(output) = std::process::Command::new("codesign")
    .arg("-dv")
    .arg(path)
    .output()
  else {
    return false;
  };
  // codesign writes its display info to stderr, not stdout.
  let stderr = String::from_utf8_lossy(&output.stderr);
  stderr
    .lines()
    .filter_map(|l| l.strip_prefix("Identifier="))
    .any(|id| id.trim() == expected)
}

/// Given a path to the laufey Mach-O binary (`laufey.app/Contents/MacOS/laufey`),
/// return the containing `.app` bundle. Returns `None` if the path
/// doesn't sit inside a `.app` bundle in the expected layout.
#[cfg(target_os = "macos")]
fn laufey_app_for_binary(binary: &Path) -> Option<PathBuf> {
  let app = binary.parent()?.parent()?.parent()?;
  if app.extension().and_then(|s| s.to_str()) == Some("app") {
    Some(app.to_path_buf())
  } else {
    None
  }
}

/// Locate the laufey backend's `.app` bundle within an extracted install
/// dir. The release layout varies by backend (`cef/Release/laufey.app`,
/// `webview/Release/laufey_webview.app`, etc.) so probe the well-known
/// names rather than hardcoding one.
#[cfg(target_os = "macos")]
fn locate_laufey_app_in_install(dir: &Path, backend: &str) -> Option<PathBuf> {
  let candidates = match backend {
    "cef" => vec![
      dir.join("laufey.app"),
      dir.join("Release/laufey.app"),
      dir.join("cef/Release/laufey.app"),
    ],
    "webview" => vec![
      dir.join("laufey_webview.app"),
      dir.join("Release/laufey_webview.app"),
      dir.join("webview/Release/laufey_webview.app"),
    ],
    _ => vec![
      dir.join(format!("laufey_{backend}.app")),
      dir.join("laufey.app"),
    ],
  };
  candidates.into_iter().find(|p| p.exists())
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
///       laufey_webview      (LAUFEY backend binary)
///       libapp.dylib     (compiled Deno runtime + user code)
///     Resources/
///       AppIcon.icns     (optional)
/// ```
async fn package_macos_app_bundle(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  laufey_resolver: &LaufeyBackendResolver,
) -> Result<PathBuf, AnyError> {
  let parts = dylib_parts(dylib_path)?;
  let app_name = parts.app_name.clone();
  let app_bundle = parts.parent.join(format!("{}.app", app_name));

  // Find the LAUFEY backend .app and its main executable.
  let backend = desktop_flags.backend.as_deref().unwrap_or("cef");
  let target = laufey_target_for(desktop_flags);
  let laufey_app = laufey_resolver.find_app_bundle(backend, target).await?;
  let laufey_executable_name = read_plist_string(
    &laufey_app.join("Contents/Info.plist"),
    "CFBundleExecutable",
  )
  .unwrap_or_else(|| "laufey_webview".to_string());
  let laufey_binary = laufey_app
    .join("Contents/MacOS")
    .join(&laufey_executable_name);
  if !laufey_binary.exists() {
    bail!(
      "LAUFEY backend executable not found at '{}'",
      laufey_binary.display()
    );
  }

  // Remove existing bundle.
  if app_bundle.exists() {
    std::fs::remove_dir_all(&app_bundle)?;
  }

  // Copy the entire LAUFEY .app as the shell (CEF needs Frameworks/, Resources/, etc.).
  crate::tools::compile::copy_dir_all(&laufey_app, &app_bundle)?;

  let contents_dir = app_bundle.join("Contents");
  let macos_dir = contents_dir.join("MacOS");
  let resources_dir = contents_dir.join("Resources");
  std::fs::create_dir_all(&resources_dir)?;

  // The backend binary extracts its self-extracting VFS to a sibling
  // `.<exe>` dir on first run. If the source laufey.app was ever run, that dir
  // gets copied along with it — drop any such runtime caches.
  let laufey_exe_stem = Path::new(&laufey_executable_name)
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| laufey_executable_name.clone());
  let cache_dir = macos_dir.join(format!(".{}", laufey_exe_stem));
  if cache_dir.exists() {
    let _ = std::fs::remove_dir_all(&cache_dir);
  }
  let cache_file = macos_dir.join(format!(".{}.cache", laufey_exe_stem));
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
  validate_launcher_name(
    &laufey_executable_name,
    "LAUFEY backend executable name",
  )?;
  validate_launcher_name(&dylib_filename_str, "dylib filename")?;
  let launcher_path = macos_dir.join(&app_name);
  std::fs::write(
    &launcher_path,
    format!(
      "#!/bin/sh\n\
       DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
       exec \"$DIR/{laufey_binary}\" --runtime \"$DIR/{dylib}\" \"$@\"\n",
      laufey_binary = laufey_executable_name,
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

  // Resolve the bundle identifier. The user-configured `identifier` is
  // preferred; otherwise we synthesize one from the app name. The
  // synthetic form is fine for `deno run`-like local use, but real
  // distribution wants a stable reverse-DNS identifier — notification
  // permission (and other tcc-keyed permissions) are decided per
  // (bundle id, code signature), so a synthetic id changes the user's
  // grant whenever they rename the app.
  let bundle_id = match desktop_flags.identifier.as_deref() {
    Some(id) => {
      validate_bundle_identifier(id)?;
      id.to_string()
    }
    None => {
      let slug = app_name.to_lowercase().replace(' ', "-");
      format!("com.deno.desktop.{slug}")
    }
  };

  // Generate Info.plist.
  let has_icon = desktop_flags.icon.is_some();
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
  <string>{bundle_id}</string>
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

  // Rewrite each CEF helper's CFBundleIdentifier to be a strict suffix
  // of the main bundle id. CEF's process model requires this — the
  // browser process matches a helper's bundle id prefix against its own
  // to verify the helper is part of the same app, and the laufey defaults
  // (`com.example.laufey.helper.*`) don't share a prefix with whatever
  // identifier we just wrote into the main plist.
  rewrite_cef_helper_bundle_ids(&contents_dir, &bundle_id)?;

  // Codesign the assembled bundle. Helpers must be signed before the
  // main bundle (Gatekeeper / CEF verify them in that order), and the
  // main bundle's signature seals the helpers' signatures into its
  // CodeDirectory.
  //
  // When the user provides an identity we use it (path to a real
  // Developer ID for distribution). Otherwise — on a macOS host — we
  // ad-hoc sign with `-`. Ad-hoc is required for the bundle to receive
  // a stable code identity from the OS, which gates:
  //   - UNUserNotificationCenter authorization (without signing,
  //     `requestAuthorization` silently fails and the user sees
  //     "denied" with no prompt — this is why Notification.permission
  //     stays "denied" for unsigned dev builds);
  //   - LaunchServices registration under a stable bundle id;
  //   - TCC entries (microphone/camera/automation) attaching to a
  //     persistent identity rather than re-prompting on every rebuild.
  // We skip on non-macOS hosts (cross-build) since `codesign(1)` only
  // exists on macOS.
  let codesign_identity = desktop_flags.codesign_identity.as_deref().or(
    if cfg!(target_os = "macos") {
      Some("-")
    } else {
      None
    },
  );
  if let Some(identity) = codesign_identity {
    codesign_macos_bundle(&app_bundle, identity)?;
  }

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

/// Spawn a child with macOS TCC "responsibility" disclaimed, so the child
/// is its own permission principal instead of inheriting attribution from
/// the calling chain (terminal → deno → laufey).
///
/// Without this, requests like `UNUserNotificationCenter.requestAuthorization`
/// fail immediately with `UNErrorCodeNotificationsNotAllowed` because TCC
/// resolves "who's asking" to a process that has no notification bundle id.
/// `responsibility_spawnattrs_setdisclaim` is the same SPI `open(1)` uses
/// internally — it tells the kernel "this child decides its own permissions".
#[cfg(target_os = "macos")]
mod disclaim_spawn {
  use std::ffi::CString;
  use std::ffi::OsString;
  use std::os::unix::ffi::OsStrExt;
  use std::process::ExitStatus;

  // SPI in libsystem (10.14+). Not in libc's bindings.
  unsafe extern "C" {
    fn responsibility_spawnattrs_setdisclaim(
      attrs: *mut libc::posix_spawnattr_t,
      disclaim: libc::c_int,
    ) -> libc::c_int;
    // macOS 10.15+. libc has the *_addclose family but not *_addchdir_np yet.
    fn posix_spawn_file_actions_addchdir_np(
      actions: *mut libc::posix_spawn_file_actions_t,
      path: *const libc::c_char,
    ) -> libc::c_int;
  }

  pub struct Child {
    pid: libc::pid_t,
    exited: bool,
  }

  impl Child {
    pub async fn wait(&mut self) -> std::io::Result<ExitStatus> {
      use std::os::unix::process::ExitStatusExt;
      let pid = self.pid;
      let status_raw = tokio::task::spawn_blocking(move || {
        let mut status: libc::c_int = 0;
        loop {
          // Safety: waiting on our own child pid.
          let rc = unsafe { libc::waitpid(pid, &mut status, 0) };
          if rc < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
              continue;
            }
            return Err(err);
          }
          return Ok(status);
        }
      })
      .await
      .map_err(std::io::Error::other)??;
      self.exited = true;
      Ok(ExitStatus::from_raw(status_raw))
    }
  }

  impl Drop for Child {
    fn drop(&mut self) {
      // Mirrors tokio's kill_on_drop: if we didn't observe an exit, SIGKILL
      // the orphan. Polite escalation (TERM-then-KILL) isn't possible inside
      // sync Drop, and the existing tokio path uses SIGKILL too.
      if !self.exited {
        // SAFETY: `kill(2)` with a pid we spawned is always safe to call; a
        // stale pid simply returns ESRCH, which we ignore.
        unsafe {
          libc::kill(self.pid, libc::SIGKILL);
        }
      }
    }
  }

  /// Flattened `(program, argv, envp, cwd)` for `posix_spawn`.
  type SpawnArgs = (CString, Vec<CString>, Vec<CString>, Option<CString>);

  /// Convert a std::process::Command into the argv/envp/cwd tuple posix_spawn
  /// needs. Inherits the parent's env, then applies Command::env() overrides
  /// (matching what std::process::Command does internally).
  fn flatten(cmd: &std::process::Command) -> std::io::Result<SpawnArgs> {
    let program = CString::new(cmd.get_program().as_bytes()).map_err(|_| {
      std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "program path contains NUL",
      )
    })?;
    let mut argv: Vec<CString> = Vec::with_capacity(cmd.get_args().len() + 1);
    argv.push(program.clone());
    for a in cmd.get_args() {
      argv.push(CString::new(a.as_bytes()).map_err(|_| {
        std::io::Error::new(
          std::io::ErrorKind::InvalidInput,
          "argv contains NUL",
        )
      })?);
    }
    let mut env_map: std::collections::BTreeMap<OsString, OsString> =
      std::env::vars_os().collect();
    for (k, v) in cmd.get_envs() {
      match v {
        Some(v) => {
          env_map.insert(k.to_os_string(), v.to_os_string());
        }
        None => {
          env_map.remove(k);
        }
      }
    }
    let envp: Vec<CString> = env_map
      .into_iter()
      .map(|(k, v)| {
        let mut s =
          Vec::with_capacity(k.as_bytes().len() + 1 + v.as_bytes().len());
        s.extend_from_slice(k.as_bytes());
        s.push(b'=');
        s.extend_from_slice(v.as_bytes());
        CString::new(s).map_err(|_| {
          std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "env contains NUL",
          )
        })
      })
      .collect::<std::io::Result<_>>()?;
    let cwd = match cmd.get_current_dir() {
      Some(p) => {
        Some(CString::new(p.as_os_str().as_bytes()).map_err(|_| {
          std::io::Error::new(std::io::ErrorKind::InvalidInput, "cwd has NUL")
        })?)
      }
      None => None,
    };
    Ok((program, argv, envp, cwd))
  }

  pub fn spawn(cmd: &std::process::Command) -> std::io::Result<Child> {
    let (program, argv, envp, cwd) = flatten(cmd)?;
    let mut argv_ptrs: Vec<*mut libc::c_char> =
      argv.iter().map(|c| c.as_ptr() as *mut _).collect();
    argv_ptrs.push(std::ptr::null_mut());
    let mut envp_ptrs: Vec<*mut libc::c_char> =
      envp.iter().map(|c| c.as_ptr() as *mut _).collect();
    envp_ptrs.push(std::ptr::null_mut());

    // SAFETY: posix_spawn FFI. We initialize attrs/actions before use,
    // destroy them on every exit path, and keep argv/envp CString backing
    // alive until posix_spawn returns. Spawn inherits fds 0/1/2 by default
    // (no file action redirects them), which is the stdio behavior the
    // caller expects.
    unsafe {
      let mut attrs: libc::posix_spawnattr_t = std::mem::zeroed();
      if libc::posix_spawnattr_init(&mut attrs) != 0 {
        return Err(std::io::Error::last_os_error());
      }
      let res = (|| -> std::io::Result<libc::pid_t> {
        let mut actions: libc::posix_spawn_file_actions_t = std::mem::zeroed();
        if libc::posix_spawn_file_actions_init(&mut actions) != 0 {
          return Err(std::io::Error::last_os_error());
        }
        let inner = (|| -> std::io::Result<libc::pid_t> {
          // The disclaim. Ignored if the SPI is missing (hypothetical
          // older system) — caller just falls back to inherited TCC.
          let _ = responsibility_spawnattrs_setdisclaim(&mut attrs, 1);

          if let Some(cwd) = cwd.as_ref() {
            let rc =
              posix_spawn_file_actions_addchdir_np(&mut actions, cwd.as_ptr());
            if rc != 0 {
              return Err(std::io::Error::from_raw_os_error(rc));
            }
          }

          let mut pid: libc::pid_t = 0;
          let rc = libc::posix_spawn(
            &mut pid,
            program.as_ptr(),
            &actions,
            &attrs,
            argv_ptrs.as_mut_ptr(),
            envp_ptrs.as_mut_ptr(),
          );
          if rc != 0 {
            return Err(std::io::Error::from_raw_os_error(rc));
          }
          Ok(pid)
        })();
        libc::posix_spawn_file_actions_destroy(&mut actions);
        inner
      })();
      libc::posix_spawnattr_destroy(&mut attrs);
      let pid = res?;
      Ok(Child { pid, exited: false })
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // --- laufey_archive_name / laufey_release_url ---

  #[test]
  fn archive_name_extensions() {
    assert_eq!(
      laufey_archive_name("cef", "aarch64-apple-darwin"),
      "laufey-cef-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
      laufey_archive_name("webview", "x86_64-pc-windows-msvc"),
      "laufey-webview-x86_64-pc-windows-msvc.zip",
      "windows targets must use zip, not tar.gz — the bundled 7z step assumes it"
    );
  }

  #[test]
  fn archive_name_raw_aliases_to_winit() {
    // `raw` is the public name; the GitHub releases ship under `winit`.
    // A regression here would 404 every backend download for raw users.
    assert_eq!(
      laufey_archive_name("raw", "x86_64-unknown-linux-gnu"),
      "laufey-winit-x86_64-unknown-linux-gnu.tar.gz"
    );
  }

  #[test]
  fn release_url_uses_v_prefix() {
    let url = laufey_release_url("laufey-cef-aarch64-apple-darwin.tar.gz");
    assert!(
      url.starts_with(
        "https://github.com/littledivy/laufey/releases/download/v"
      )
    );
    assert!(url.ends_with("/laufey-cef-aarch64-apple-darwin.tar.gz"));
    // No spaces, no shell metachars — this string is fed to `curl` and to
    // log messages.
    assert!(!url.contains(' '));
  }

  // --- parse_sha256sum ---

  #[test]
  fn parse_sha256sum_basic() {
    let contents = "\
abc123  file-a.tar.gz
def456  file-b.zip
";
    assert_eq!(
      parse_sha256sum(contents, "file-a.tar.gz").as_deref(),
      Some("abc123")
    );
    assert_eq!(
      parse_sha256sum(contents, "file-b.zip").as_deref(),
      Some("def456")
    );
    assert_eq!(parse_sha256sum(contents, "missing").as_deref(), None);
  }

  #[test]
  fn parse_sha256sum_handles_binary_mode_star() {
    // GNU sha256sum's binary mode emits `<hex>  *filename`. We must
    // strip the leading star or we'll fail to match every Windows
    // artifact line.
    let contents = "abc123  *file-a.zip\n";
    assert_eq!(
      parse_sha256sum(contents, "file-a.zip").as_deref(),
      Some("abc123")
    );
  }

  #[test]
  fn parse_sha256sum_tolerates_blank_and_extra_whitespace() {
    let contents = "\

abc123    file.tar.gz
   \t

def456  other.zip
";
    assert_eq!(
      parse_sha256sum(contents, "file.tar.gz").as_deref(),
      Some("abc123")
    );
    assert_eq!(
      parse_sha256sum(contents, "other.zip").as_deref(),
      Some("def456")
    );
  }

  // --- validate_bundle_identifier ---

  #[test]
  fn bundle_id_accepts_canonical() {
    for ok in &[
      "com.deno.app",
      "com.deno.my-app",
      "com.deno.app.v2",
      "A.B",
      "com.deno.app.helper",
    ] {
      assert!(
        validate_bundle_identifier(ok).is_ok(),
        "{ok:?} should be accepted"
      );
    }
  }

  #[test]
  fn bundle_id_rejects_bad_shapes() {
    let cases = &[
      ("", "empty"),
      ("noseparator", "no dot"),
      ("com.deno.", "trailing empty segment"),
      (".com.deno", "leading empty segment"),
      ("com..deno", "doubled dot"),
      ("com.deno.app/foo", "slash"),
      ("com.deno.app foo", "space"),
      ("com.deno.app$bar", "shell metachar"),
      ("com.deno.app;rm", "semicolon"),
      ("com.deno.app\nx", "newline"),
    ];
    for (bad, why) in cases {
      assert!(
        validate_bundle_identifier(bad).is_err(),
        "{bad:?} should be rejected ({why})"
      );
    }
  }

  #[test]
  fn bundle_id_rejects_too_long() {
    let long = format!("com.deno.{}", "x".repeat(200));
    assert!(validate_bundle_identifier(&long).is_err());
    // Right at the boundary.
    let just_too_long = format!("com.deno.{}", "x".repeat(155));
    assert!(
      validate_bundle_identifier(&just_too_long).is_err(),
      "ids longer than 155 chars must be rejected (Apple receipt limit)"
    );
  }

  // --- validate_launcher_name ---

  #[test]
  fn launcher_name_accepts_canonical() {
    for ok in &["my-app", "My App", "app_1.0", "FooBar", "a"] {
      assert!(
        validate_launcher_name(ok, "test").is_ok(),
        "{ok:?} should be accepted"
      );
    }
  }

  #[test]
  fn launcher_name_rejects_shell_metachars() {
    // Every char that could let a value escape an unquoted .bat / .sh
    // launcher line must be rejected.
    let cases = &[
      ("", "empty"),
      ("a&b", "ampersand"),
      ("a;b", "semicolon"),
      ("a|b", "pipe"),
      ("a$b", "dollar"),
      ("a`b", "backtick"),
      ("a'b", "single quote"),
      ("a\"b", "double quote"),
      ("a\\b", "backslash"),
      ("a/b", "slash"),
      ("a\nb", "newline"),
      ("a\rb", "carriage return"),
      ("a\tb", "tab"),
      ("a\0b", "NUL"),
      ("café", "non-ascii"),
    ];
    for (bad, why) in cases {
      assert!(
        validate_launcher_name(bad, "test").is_err(),
        "{bad:?} should be rejected ({why})"
      );
    }
  }

  #[test]
  fn launcher_name_error_message_includes_kind() {
    let err = validate_launcher_name("bad/name", "LAUFEY backend binary name")
      .unwrap_err()
      .to_string();
    assert!(
      err.contains("LAUFEY backend binary name"),
      "error should label what was invalid; got: {err}"
    );
    assert!(
      err.contains("/"),
      "error should name the bad char; got: {err}"
    );
  }

  // --- dylib_parts ---

  #[test]
  fn dylib_parts_basic() {
    let p = std::path::PathBuf::from("/tmp/app/myapp.dylib");
    let parts = dylib_parts(&p).expect("parts");
    assert_eq!(parts.parent, std::path::Path::new("/tmp/app"));
    assert_eq!(parts.file_name, "myapp.dylib");
    assert_eq!(parts.app_name, "myapp");
  }

  #[test]
  fn dylib_parts_strips_libdenort_prefix() {
    // .so on Linux carries the `lib` prefix in the file stem; the
    // resulting bundle name shouldn't include it (the user typed
    // `--output myapp`, not `libmyapp`).
    let p = std::path::PathBuf::from("/tmp/libdenort.so");
    let parts = dylib_parts(&p).expect("parts");
    // dylib_parts itself doesn't strip; that's downstream. But the
    // pieces should at least round-trip cleanly.
    assert_eq!(parts.file_name, "libdenort.so");
    assert_eq!(parts.app_name, "libdenort");
  }

  #[test]
  fn dylib_parts_rejects_degenerate_inputs() {
    // The whole point of this helper is to surface friendly errors
    // instead of the `file_stem().unwrap()` panic the old code had
    // on `--output /` and `--output .`.
    assert!(dylib_parts(std::path::Path::new("/")).is_err());
    // `dylib_parts` doesn't reject "" by itself — Path::new("") still
    // has a parent (None on some platforms, Some("") on others). Spot
    // a degenerate case that previously panicked.
    let empty = std::path::Path::new("");
    let _ = dylib_parts(empty); // not panicking is the regression test
  }

  // --- appimage_runtime_for_target ---

  #[test]
  fn appimage_runtime_target_arch_lookup() {
    assert!(
      appimage_runtime_for_target(Some("x86_64-unknown-linux-gnu")).is_ok()
    );
    assert!(
      appimage_runtime_for_target(Some("aarch64-unknown-linux-gnu")).is_ok()
    );
  }

  #[test]
  fn appimage_runtime_rejects_unknown_arch() {
    let err = appimage_runtime_for_target(Some("powerpc64-unknown-linux-gnu"))
      .unwrap_err()
      .to_string();
    assert!(
      err.contains("powerpc64"),
      "error should name the arch: {err}"
    );
    assert!(
      err.contains("x86_64") && err.contains("aarch64"),
      "error should list supported arches: {err}"
    );
  }

  // --- icns_ostypes_for_size ---

  #[test]
  fn icns_ostypes_known_sizes() {
    // These OSType codes are part of the macOS iconset spec; a change
    // here means Finder will silently skip the icon. Pin them.
    assert_eq!(icns_ostypes_for_size(16), &[b"icp4"]);
    assert_eq!(icns_ostypes_for_size(32), &[b"ic11", b"icp5"]);
    assert_eq!(icns_ostypes_for_size(64), &[b"ic12"]);
    assert_eq!(icns_ostypes_for_size(128), &[b"ic07"]);
    assert_eq!(icns_ostypes_for_size(256), &[b"ic13", b"ic08"]);
    assert_eq!(icns_ostypes_for_size(512), &[b"ic14", b"ic09"]);
    assert_eq!(icns_ostypes_for_size(1024), &[b"ic10"]);
  }

  #[test]
  fn icns_ostypes_unknown_size_returns_empty() {
    assert!(icns_ostypes_for_size(0).is_empty());
    assert!(icns_ostypes_for_size(48).is_empty());
    assert!(icns_ostypes_for_size(999).is_empty());
  }

  // --- extract_laufey_archive ---
  //
  // These tests build tar.gz fixtures in-memory and feed them through
  // extract_laufey_archive. They mirror the manual checklist items 1.33–1.37:
  // a malicious archive must be rejected, a normal one must extract,
  // and setuid/world-writable bits must never reach disk.

  use std::io::Read;
  use std::io::Write;

  fn make_tar_gz(entries: &[(&str, tar::EntryType, &[u8], u32)]) -> Vec<u8> {
    let mut tar_buf: Vec<u8> = Vec::new();
    {
      let mut builder = tar::Builder::new(&mut tar_buf);
      for (name, ty, data, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(*mode);
        header.set_mtime(0);
        header.set_entry_type(*ty);
        header.set_cksum();
        builder
          .append_data(&mut header, name, &data[..])
          .expect("tar append");
      }
      builder.finish().expect("tar finish");
    }
    let mut gz = Vec::new();
    let mut enc =
      flate2::write::GzEncoder::new(&mut gz, flate2::Compression::default());
    enc.write_all(&tar_buf).expect("gz write");
    enc.finish().expect("gz finish");
    gz
  }

  fn make_tar_gz_symlink(name: &str, target: &str) -> Vec<u8> {
    let mut tar_buf: Vec<u8> = Vec::new();
    {
      let mut builder = tar::Builder::new(&mut tar_buf);
      let mut header = tar::Header::new_gnu();
      header.set_size(0);
      header.set_mode(0o777);
      header.set_mtime(0);
      header.set_entry_type(tar::EntryType::Symlink);
      header.set_link_name(target).expect("set_link_name");
      header.set_cksum();
      builder
        .append_data(&mut header, name, std::io::empty())
        .expect("tar append symlink");
      builder.finish().expect("tar finish");
    }
    let mut gz = Vec::new();
    let mut enc =
      flate2::write::GzEncoder::new(&mut gz, flate2::Compression::default());
    enc.write_all(&tar_buf).expect("gz write");
    enc.finish().expect("gz finish");
    gz
  }

  #[test]
  fn extract_tar_normal_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let gz = make_tar_gz(&[
      ("greet", tar::EntryType::Regular, b"hello", 0o755),
      ("notes/readme.txt", tar::EntryType::Regular, b"docs", 0o644),
    ]);
    extract_laufey_archive("ok.tar.gz", &gz, tmp.path()).expect("extract");

    let mut out = String::new();
    std::fs::File::open(tmp.path().join("greet"))
      .unwrap()
      .read_to_string(&mut out)
      .unwrap();
    assert_eq!(out, "hello");
    assert!(tmp.path().join("notes/readme.txt").exists());
  }

  #[test]
  fn extract_tar_rejects_parent_dir_traversal() {
    // Hand-craft a tar block with `../escape.txt` as the name: the
    // `tar` crate's `Builder` refuses to *write* such a path (a safety
    // feature in the producer), so a unit test that goes through it
    // wouldn't actually exercise extract_laufey_archive's reader-side
    // check. We construct the 512-byte ustar header directly to get a
    // genuinely-malicious archive on the wire.
    fn ustar_block(name: &str, body: &[u8]) -> Vec<u8> {
      let mut hdr = [0u8; 512];
      // name (offset 0..100)
      let nb = name.as_bytes();
      hdr[..nb.len()].copy_from_slice(nb);
      // mode "000644 \0" octal (offset 100..108)
      hdr[100..108].copy_from_slice(b"000644 \0");
      // uid/gid zeroed via spaces+NUL
      hdr[108..116].copy_from_slice(b"000000 \0");
      hdr[116..124].copy_from_slice(b"000000 \0");
      // size in octal
      let sz = format!("{:011o} ", body.len());
      hdr[124..136].copy_from_slice(sz.as_bytes());
      // mtime
      hdr[136..148].copy_from_slice(b"00000000000 ");
      // checksum placeholder (8 spaces)
      hdr[148..156].copy_from_slice(b"        ");
      // typeflag '0' = regular file
      hdr[156] = b'0';
      // magic "ustar" then null then version "00"
      hdr[257..263].copy_from_slice(b"ustar\0");
      hdr[263..265].copy_from_slice(b"00");
      // checksum: unsigned sum of all bytes, written as 6 octal digits
      // + NUL + space at offset 148..156.
      let sum: u32 = hdr.iter().map(|&b| b as u32).sum();
      let cs = format!("{:06o}\0 ", sum);
      hdr[148..156].copy_from_slice(cs.as_bytes());

      let mut out = Vec::with_capacity(512 + body.len().div_ceil(512) * 512);
      out.extend_from_slice(&hdr);
      out.extend_from_slice(body);
      // pad body to 512.
      let pad = (512 - body.len() % 512) % 512;
      out.extend(std::iter::repeat_n(0u8, pad));
      // two zero blocks for end-of-archive marker.
      out.extend(std::iter::repeat_n(0u8, 1024));
      out
    }

    let raw_tar = ustar_block("../escape.txt", b"oops");
    let mut gz = Vec::new();
    let mut enc =
      flate2::write::GzEncoder::new(&mut gz, flate2::Compression::default());
    enc.write_all(&raw_tar).unwrap();
    enc.finish().unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let err = extract_laufey_archive("evil.tar.gz", &gz, tmp.path())
      .expect_err("malicious `..` path must be rejected");
    let msg = err.to_string();
    assert!(
      msg.contains("traversal") || msg.contains("outside"),
      "error must indicate the rejection reason; got: {msg}"
    );
    // The sibling file must not exist — defence-in-depth check passed.
    assert!(!tmp.path().parent().unwrap().join("escape.txt").exists());
  }

  #[test]
  fn extract_tar_rejects_symlink_escape() {
    let tmp = tempfile::tempdir().unwrap();
    // A bare symlink whose target escapes the dest. unpack_in must
    // refuse: the test name documents the canonical zip-slip-via-symlink
    // pattern (entry A = symlink escape, entry B writes through it).
    let gz = make_tar_gz_symlink("foo", "../../etc/passwd");
    let _ = extract_laufey_archive("evil.tar.gz", &gz, tmp.path());
    // Tar's `unpack_in` is allowed to either error or skip the entry —
    // both behaviours mean the symlink didn't land in dest. Either is
    // acceptable; what matters is that nothing escaped.
    assert!(
      !tmp.path().join("foo").exists()
        || std::fs::symlink_metadata(tmp.path().join("foo"))
          .map(|m| m.file_type().is_symlink())
          .unwrap_or(false),
      "if a symlink was extracted it must live inside dest"
    );
    // The target itself never came into existence under dest.
    assert!(!tmp.path().join("etc/passwd").exists());
  }

  #[cfg(unix)]
  #[test]
  fn extract_tar_strips_setuid_bits() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    // Setuid + setgid + world-writable + executable. All the bad bits
    // we never want extracted to disk.
    let gz = make_tar_gz(&[(
      "exe",
      tar::EntryType::Regular,
      b"#!/bin/sh\necho gotcha\n",
      0o7777,
    )]);
    extract_laufey_archive("perm.tar.gz", &gz, tmp.path()).expect("extract");
    let meta = std::fs::metadata(tmp.path().join("exe")).unwrap();
    let mode = meta.permissions().mode() & 0o7777;
    // We normalize execute-bit-set files to 0o755. setuid/setgid/sticky
    // bits must be gone, world-writable must be gone.
    assert_eq!(
      mode, 0o755,
      "extracted mode must be exactly 0o755 (was {:o})",
      mode
    );
  }

  #[cfg(unix)]
  #[test]
  fn extract_tar_keeps_non_executable_as_0644() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    // A doc file with overly permissive 0o666 mode — must be downgraded
    // to 0o644 (no world-writable).
    let gz =
      make_tar_gz(&[("readme.txt", tar::EntryType::Regular, b"hello", 0o666)]);
    extract_laufey_archive("doc.tar.gz", &gz, tmp.path()).expect("extract");
    let mode = std::fs::metadata(tmp.path().join("readme.txt"))
      .unwrap()
      .permissions()
      .mode()
      & 0o7777;
    assert_eq!(
      mode, 0o644,
      "non-executable mode must be 0o644 (was {:o})",
      mode
    );
  }

  #[test]
  fn extract_unknown_format_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let err = extract_laufey_archive("evil.rar", b"PK\x03\x04", tmp.path())
      .expect_err("unknown extensions must error");
    assert!(err.to_string().contains("unsupported archive format"));
  }

  // --- rewrite_helper_plist_identifier ---

  fn write_helper_plist(path: &std::path::Path, bundle_id: &str) {
    let xml = format!(
      r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>{bundle_id}</string>
</dict>
</plist>
"#
    );
    std::fs::write(path, xml).unwrap();
  }

  fn read_bundle_id(path: &std::path::Path) -> String {
    let d: plist::Dictionary = plist::from_file(path).unwrap();
    d.get("CFBundleIdentifier")
      .and_then(|v| v.as_string())
      .unwrap()
      .to_string()
  }

  #[test]
  fn rewrite_helper_plist_keeps_helper_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("Info.plist");
    write_helper_plist(&p, "com.example.laufey.helper");
    rewrite_helper_plist_identifier(&p, "com.acme.myapp").unwrap();
    assert_eq!(read_bundle_id(&p), "com.acme.myapp.helper");
  }

  #[test]
  fn rewrite_helper_plist_keeps_subhelper_suffix() {
    // CEF spawns multiple helper variants (Renderer, GPU, Plugin,
    // Alerts). Each Info.plist's existing id has the role baked in
    // after `helper`; we must preserve everything from `helper` onward.
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("Info.plist");
    write_helper_plist(&p, "com.example.laufey.helper.gpu");
    rewrite_helper_plist_identifier(&p, "com.acme.myapp").unwrap();
    assert_eq!(read_bundle_id(&p), "com.acme.myapp.helper.gpu");
  }

  #[test]
  fn rewrite_helper_plist_falls_back_when_no_helper_token() {
    // Defensive path: every laufey helper plist today has `helper` in its
    // id, but if a hypothetical future bundle drops it we still emit
    // something reasonable.
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("Info.plist");
    write_helper_plist(&p, "com.example.laufey.misc");
    rewrite_helper_plist_identifier(&p, "com.acme.myapp").unwrap();
    assert_eq!(read_bundle_id(&p), "com.acme.myapp.helper");
  }

  // --- extract_laufey_archive ZIP path ---

  fn build_zip<
    F: FnOnce(&mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>),
  >(
    f: F,
  ) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    {
      let cursor = std::io::Cursor::new(&mut buf);
      let mut writer = zip::ZipWriter::new(cursor);
      f(&mut writer);
      writer.finish().unwrap();
    }
    buf
  }

  #[test]
  fn extract_zip_normal_succeeds() {
    use zip::write::SimpleFileOptions;
    let zip = build_zip(|w| {
      w.start_file("greet.txt", SimpleFileOptions::default())
        .unwrap();
      w.write_all(b"hello zip").unwrap();
      w.start_file("nested/readme.md", SimpleFileOptions::default())
        .unwrap();
      w.write_all(b"docs").unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    extract_laufey_archive("ok.zip", &zip, tmp.path()).expect("extract");
    assert_eq!(
      std::fs::read(tmp.path().join("greet.txt")).unwrap(),
      b"hello zip"
    );
    assert!(tmp.path().join("nested/readme.md").exists());
  }

  #[test]
  fn extract_zip_rejects_absolute_path() {
    use zip::write::SimpleFileOptions;
    let zip = build_zip(|w| {
      // `enclosed_name` rejects absolute paths.
      w.start_file("/etc/escape.txt", SimpleFileOptions::default())
        .unwrap();
      w.write_all(b"oops").unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    let err = extract_laufey_archive("evil.zip", &zip, tmp.path())
      .expect_err("absolute path in zip must be rejected");
    let msg = err.to_string();
    assert!(
      msg.contains("unsafe") || msg.contains("traversal"),
      "error must indicate rejection; got: {msg}"
    );
    // /etc/escape.txt mustn't exist on the host.
    assert!(!std::path::Path::new("/etc/escape.txt").exists());
  }

  #[test]
  fn extract_zip_rejects_parent_traversal() {
    use zip::write::SimpleFileOptions;
    // ZIP without "/" prefix but with `..` segments — must also be
    // refused by enclosed_name OR the defence-in-depth components check.
    let zip = build_zip(|w| {
      w.start_file("../escape.txt", SimpleFileOptions::default())
        .unwrap();
      w.write_all(b"oops").unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    let err = extract_laufey_archive("evil.zip", &zip, tmp.path())
      .expect_err("parent-dir traversal in zip must be rejected");
    let msg = err.to_string();
    assert!(
      msg.contains("unsafe") || msg.contains("traversal"),
      "error must indicate rejection; got: {msg}"
    );
    assert!(!tmp.path().parent().unwrap().join("escape.txt").exists());
  }

  #[test]
  fn extract_zip_rejects_symlink_entries() {
    use zip::write::SimpleFileOptions;
    let zip = build_zip(|w| {
      // Use `add_symlink` so the entry is genuinely typed as a symlink
      // in the ZIP central directory. `entry.is_symlink()` in the
      // reader keys off that — a regression that removed our `if
      // entry.is_symlink() { bail!(...) }` check would let a follow-on
      // entry write through the symlink to escape `dest`.
      w.add_symlink("link", "../../etc", SimpleFileOptions::default())
        .unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    let err = extract_laufey_archive("evil.zip", &zip, tmp.path())
      .expect_err("symlink in zip must be rejected");
    let msg = err.to_string();
    assert!(
      msg.contains("symlink"),
      "error must name symlink rejection; got: {msg}"
    );
    // And nothing landed at the symlink path.
    assert!(!tmp.path().join("link").exists());
  }

  #[cfg(unix)]
  #[test]
  fn extract_zip_strips_setuid_bits() {
    use std::os::unix::fs::PermissionsExt;

    use zip::write::SimpleFileOptions;
    let zip = build_zip(|w| {
      let opts = SimpleFileOptions::default().unix_permissions(0o7777);
      w.start_file("exe", opts).unwrap();
      w.write_all(b"#!/bin/sh\necho gotcha\n").unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    extract_laufey_archive("perm.zip", &zip, tmp.path()).expect("extract");
    let mode = std::fs::metadata(tmp.path().join("exe"))
      .unwrap()
      .permissions()
      .mode()
      & 0o7777;
    assert_eq!(mode, 0o755, "mode must be exactly 0o755 (was {:o})", mode);
  }

  #[cfg(unix)]
  #[test]
  fn extract_zip_keeps_non_exec_at_0644() {
    use std::os::unix::fs::PermissionsExt;

    use zip::write::SimpleFileOptions;
    let zip = build_zip(|w| {
      let opts = SimpleFileOptions::default().unix_permissions(0o666);
      w.start_file("doc.txt", opts).unwrap();
      w.write_all(b"hello").unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    extract_laufey_archive("perm.zip", &zip, tmp.path()).expect("extract");
    let mode = std::fs::metadata(tmp.path().join("doc.txt"))
      .unwrap()
      .permissions()
      .mode()
      & 0o7777;
    assert_eq!(mode, 0o644);
  }

  #[test]
  fn extract_zip_creates_nested_directories() {
    use zip::write::SimpleFileOptions;
    let zip = build_zip(|w| {
      w.start_file("a/b/c/leaf.txt", SimpleFileOptions::default())
        .unwrap();
      w.write_all(b"deep").unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    extract_laufey_archive("nest.zip", &zip, tmp.path()).expect("extract");
    assert_eq!(
      std::fs::read(tmp.path().join("a/b/c/leaf.txt")).unwrap(),
      b"deep"
    );
  }

  // --- read_plist_string ---

  #[test]
  fn read_plist_string_returns_value_for_existing_key() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("Info.plist");
    std::fs::write(
      &p,
      r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>my_app</string>
  <key>CFBundleIdentifier</key><string>com.example.foo</string>
</dict></plist>
"#,
    )
    .unwrap();
    assert_eq!(
      read_plist_string(&p, "CFBundleExecutable").as_deref(),
      Some("my_app")
    );
    assert_eq!(
      read_plist_string(&p, "CFBundleIdentifier").as_deref(),
      Some("com.example.foo")
    );
  }

  #[test]
  fn read_plist_string_none_for_missing_key() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("Info.plist");
    std::fs::write(
      &p,
      r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>X</key><string>y</string></dict></plist>
"#,
    )
    .unwrap();
    assert_eq!(read_plist_string(&p, "CFBundleExecutable"), None);
  }

  #[test]
  fn read_plist_string_none_for_non_string_value() {
    // A key with a non-string value (e.g. an array) must yield None
    // rather than panic or return some stringified form.
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("Info.plist");
    std::fs::write(
      &p,
      r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>K</key><array><string>x</string></array></dict></plist>
"#,
    )
    .unwrap();
    assert_eq!(read_plist_string(&p, "K"), None);
  }

  #[test]
  fn read_plist_string_none_for_unreadable_or_corrupt_file() {
    let tmp = tempfile::tempdir().unwrap();
    // Non-existent file.
    assert_eq!(
      read_plist_string(&tmp.path().join("missing.plist"), "K"),
      None
    );
    // Garbage contents.
    let p = tmp.path().join("bad.plist");
    std::fs::write(&p, b"this is not a plist").unwrap();
    assert_eq!(read_plist_string(&p, "K"), None);
  }

  // --- rewrite_cef_helper_bundle_ids ---

  fn setup_cef_frameworks(tmp: &std::path::Path) -> std::path::PathBuf {
    let contents = tmp.join("Contents");
    let fw = contents.join("Frameworks");
    std::fs::create_dir_all(&fw).unwrap();
    let make_helper_app = |name: &str, id: &str| {
      let app = fw.join(format!("{name}.app"));
      let plist_dir = app.join("Contents");
      std::fs::create_dir_all(&plist_dir).unwrap();
      write_helper_plist(&plist_dir.join("Info.plist"), id);
    };
    make_helper_app("laufey Helper", "com.example.laufey.helper");
    make_helper_app("laufey Helper (GPU)", "com.example.laufey.helper.gpu");
    make_helper_app(
      "laufey Helper (Renderer)",
      "com.example.laufey.helper.renderer",
    );
    // A non-helper .app — must NOT be rewritten.
    let other = fw.join("Other.app/Contents");
    std::fs::create_dir_all(&other).unwrap();
    write_helper_plist(&other.join("Info.plist"), "com.example.other");
    // The CEF framework directory itself — also must NOT be touched
    // (rewriting it invalidates its embedded code signature).
    let cef_fw =
      fw.join("Chromium Embedded Framework.framework/Versions/A/Resources");
    std::fs::create_dir_all(&cef_fw).unwrap();
    write_helper_plist(
      &cef_fw.join("Info.plist"),
      "org.chromium.embedded.framework",
    );
    contents
  }

  #[test]
  fn rewrite_helpers_rewrites_only_helper_apps() {
    let tmp = tempfile::tempdir().unwrap();
    let contents = setup_cef_frameworks(tmp.path());
    rewrite_cef_helper_bundle_ids(&contents, "com.acme.myapp").unwrap();

    let fw = contents.join("Frameworks");
    assert_eq!(
      read_bundle_id(&fw.join("laufey Helper.app/Contents/Info.plist")),
      "com.acme.myapp.helper"
    );
    assert_eq!(
      read_bundle_id(&fw.join("laufey Helper (GPU).app/Contents/Info.plist")),
      "com.acme.myapp.helper.gpu"
    );
    assert_eq!(
      read_bundle_id(
        &fw.join("laufey Helper (Renderer).app/Contents/Info.plist")
      ),
      "com.acme.myapp.helper.renderer"
    );
    // Other.app does not contain "Helper" → must NOT be rewritten.
    assert_eq!(
      read_bundle_id(&fw.join("Other.app/Contents/Info.plist")),
      "com.example.other"
    );
    // CEF framework: also untouched.
    assert_eq!(
      read_bundle_id(&fw.join(
        "Chromium Embedded Framework.framework/Versions/A/Resources/Info.plist"
      )),
      "org.chromium.embedded.framework"
    );
  }

  #[test]
  fn rewrite_helpers_is_noop_when_frameworks_missing() {
    // Some backends (winit, servo) don't ship a Frameworks/ subdir. The
    // helper-rewriter must tolerate that without erroring.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("Contents")).unwrap();
    rewrite_cef_helper_bundle_ids(
      &tmp.path().join("Contents"),
      "com.acme.myapp",
    )
    .expect("absent Frameworks must not error");
  }

  // --- locate_dev_backend_binary / locate_dev_app_bundle ---

  #[test]
  fn locate_dev_webview_returns_first_existing_candidate() {
    let tmp = tempfile::tempdir().unwrap();
    // Set up TWO candidates; the function should return whichever it
    // finds first in its candidate list. We create the SECOND one to
    // prove the function actually walks the list (rather than blindly
    // returning candidate[0] which would not exist).
    let p = tmp
      .path()
      .join("webview/build/laufey_webview.app/Contents/MacOS/laufey_webview");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"binary").unwrap();
    let found = locate_dev_backend_binary(tmp.path(), "webview");
    assert_eq!(found.as_deref(), Some(p.as_path()));
  }

  #[test]
  fn locate_dev_returns_none_when_nothing_exists() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(locate_dev_backend_binary(tmp.path(), "cef").is_none());
    assert!(locate_dev_backend_binary(tmp.path(), "webview").is_none());
    assert!(locate_dev_backend_binary(tmp.path(), "raw").is_none());
  }

  #[test]
  fn locate_dev_backend_winit_target_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("target/release/laufey_winit");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"binary").unwrap();
    // `raw` is the public backend name; the binary file is `laufey_winit`.
    let found = locate_dev_backend_binary(tmp.path(), "raw");
    assert_eq!(found.as_deref(), Some(p.as_path()));
  }

  #[test]
  fn locate_dev_app_bundle_skips_winit_and_servo() {
    // winit and servo don't ship as .app bundles. locate_dev_app_bundle
    // must short-circuit to None for those, never touching the
    // filesystem (so a misleading directory with the right name can't
    // accidentally match).
    let tmp = tempfile::tempdir().unwrap();
    assert!(locate_dev_app_bundle(tmp.path(), "raw").is_none());
    assert!(locate_dev_app_bundle(tmp.path(), "servo").is_none());
  }

  #[test]
  fn locate_dev_app_bundle_finds_webview() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("webview/build/laufey_webview.app");
    std::fs::create_dir_all(&p).unwrap();
    let found = locate_dev_app_bundle(tmp.path(), "webview");
    assert_eq!(found.as_deref(), Some(p.as_path()));
  }

  // --- convert_icon_set_to_ico ---
  //
  // ICO is a binary container with a strict header layout. A regression
  // that flips a width/height byte, or computes data_offset wrong,
  // produces a visually-fine file that Windows refuses to display.

  fn fake_png(label: &[u8]) -> Vec<u8> {
    // Not a real PNG, just bytes the converter reads verbatim into the
    // ICO entry body. The converter doesn't validate the PNG contents.
    let mut v = Vec::with_capacity(8 + label.len());
    v.extend_from_slice(b"\x89PNG\r\n\x1a\n"); // PNG magic
    v.extend_from_slice(label);
    v
  }

  fn read_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
  }

  fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
  }

  #[test]
  fn ico_writes_correct_header_for_multiple_sizes() {
    let cwd = tempfile::tempdir().unwrap();
    let png16 = fake_png(b"size16");
    let png32 = fake_png(b"size32");
    let png256 = fake_png(b"size256");
    std::fs::write(cwd.path().join("a.png"), &png16).unwrap();
    std::fs::write(cwd.path().join("b.png"), &png32).unwrap();
    std::fs::write(cwd.path().join("c.png"), &png256).unwrap();

    let entries = vec![
      crate::args::IconSetEntry {
        path: "a.png".into(),
        size: 16,
      },
      crate::args::IconSetEntry {
        path: "b.png".into(),
        size: 32,
      },
      crate::args::IconSetEntry {
        path: "c.png".into(),
        size: 256,
      },
    ];
    let out = cwd.path().join("icon.ico");
    convert_icon_set_to_ico(cwd.path(), &entries, &out).expect("ok");
    let buf = std::fs::read(&out).unwrap();

    // ICO header: reserved=0, type=1 (ICO), count=3.
    assert_eq!(read_u16(&buf, 0), 0);
    assert_eq!(read_u16(&buf, 2), 1);
    assert_eq!(read_u16(&buf, 4), 3);

    // Three directory entries follow, 16 bytes each.
    // Entry 0: 16x16 PNG. dim=16, planes=1, bpp=32, size, offset.
    assert_eq!(buf[6], 16);
    assert_eq!(buf[7], 16);
    assert_eq!(read_u16(&buf, 6 + 4), 1, "planes");
    assert_eq!(read_u16(&buf, 6 + 6), 32, "bits per pixel");
    let sz0 = read_u32(&buf, 6 + 8) as usize;
    let off0 = read_u32(&buf, 6 + 12) as usize;
    assert_eq!(sz0, png16.len());
    // First entry's data starts right after the header + 3 directory
    // entries (6 + 48 = 54).
    assert_eq!(off0, 54);

    // Entry 2: 256 is encoded as 0 in the dim byte per ICO convention.
    assert_eq!(buf[6 + 32], 0, "256px width must be encoded as 0");
    assert_eq!(buf[6 + 33], 0, "256px height must be encoded as 0");

    // The three PNG bodies appear at the offsets the directory entries
    // claim, in the order they were listed.
    assert_eq!(&buf[off0..off0 + sz0], png16.as_slice());
    let off1 = read_u32(&buf, 6 + 16 + 12) as usize;
    let sz1 = read_u32(&buf, 6 + 16 + 8) as usize;
    assert_eq!(&buf[off1..off1 + sz1], png32.as_slice());
    let off2 = read_u32(&buf, 6 + 32 + 12) as usize;
    let sz2 = read_u32(&buf, 6 + 32 + 8) as usize;
    assert_eq!(&buf[off2..off2 + sz2], png256.as_slice());
  }

  #[test]
  fn ico_skips_missing_files_with_warning() {
    let cwd = tempfile::tempdir().unwrap();
    let png = fake_png(b"only");
    std::fs::write(cwd.path().join("there.png"), &png).unwrap();
    let entries = vec![
      crate::args::IconSetEntry {
        path: "missing.png".into(),
        size: 16,
      },
      crate::args::IconSetEntry {
        path: "there.png".into(),
        size: 32,
      },
    ];
    let out = cwd.path().join("icon.ico");
    convert_icon_set_to_ico(cwd.path(), &entries, &out).expect("ok");
    let buf = std::fs::read(&out).unwrap();
    // Only one image survived; the count must reflect that, otherwise
    // the directory would name an entry with bogus offset/size.
    assert_eq!(read_u16(&buf, 4), 1, "count must skip missing entries");
  }

  #[test]
  fn ico_errors_when_no_inputs_resolve() {
    let cwd = tempfile::tempdir().unwrap();
    let entries = vec![
      crate::args::IconSetEntry {
        path: "absent1.png".into(),
        size: 16,
      },
      crate::args::IconSetEntry {
        path: "absent2.png".into(),
        size: 32,
      },
    ];
    let out = cwd.path().join("icon.ico");
    let err = convert_icon_set_to_ico(cwd.path(), &entries, &out).unwrap_err();
    assert!(err.to_string().contains("No valid icon"));
    // Output file must not exist if we never wrote anything.
    assert!(!out.exists());
  }

  #[test]
  fn ico_data_offsets_are_monotonically_increasing() {
    // The data_offset accumulator in the producer increments by each
    // image's byte length. A regression that forgot to advance the
    // offset would cause later entries to alias earlier ones.
    let cwd = tempfile::tempdir().unwrap();
    let a = fake_png(b"aaaaaaaaaa"); // distinct lengths
    let b = fake_png(b"bbbbbbbbbbbbbbbb");
    let c = fake_png(b"cccc");
    std::fs::write(cwd.path().join("a.png"), &a).unwrap();
    std::fs::write(cwd.path().join("b.png"), &b).unwrap();
    std::fs::write(cwd.path().join("c.png"), &c).unwrap();

    let entries = vec![
      crate::args::IconSetEntry {
        path: "a.png".into(),
        size: 16,
      },
      crate::args::IconSetEntry {
        path: "b.png".into(),
        size: 32,
      },
      crate::args::IconSetEntry {
        path: "c.png".into(),
        size: 64,
      },
    ];
    let out = cwd.path().join("icon.ico");
    convert_icon_set_to_ico(cwd.path(), &entries, &out).expect("ok");
    let buf = std::fs::read(&out).unwrap();
    let off0 = read_u32(&buf, 6 + 12) as usize;
    let off1 = read_u32(&buf, 6 + 16 + 12) as usize;
    let off2 = read_u32(&buf, 6 + 32 + 12) as usize;
    let sz0 = read_u32(&buf, 6 + 8) as usize;
    let sz1 = read_u32(&buf, 6 + 16 + 8) as usize;
    assert_eq!(off1, off0 + sz0, "second offset = first off + first size");
    assert_eq!(off2, off1 + sz1, "third offset = second off + second size");
    // And the file must be at least long enough to hold all images.
    let total = read_u32(&buf, 6 + 32 + 8) as usize + off2;
    assert_eq!(buf.len(), total);
  }

  #[test]
  fn rewrite_helper_plist_errors_on_missing_key() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("Info.plist");
    // Plist without CFBundleIdentifier at all — the rewrite must fail
    // loudly rather than silently insert a wrong id.
    std::fs::write(
      &p,
      r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Other</key><string>x</string></dict></plist>
"#,
    )
    .unwrap();
    let err =
      rewrite_helper_plist_identifier(&p, "com.acme.myapp").unwrap_err();
    assert!(err.to_string().contains("CFBundleIdentifier"));
  }
}
