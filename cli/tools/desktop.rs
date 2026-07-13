// Copyright 2018-2026 the Deno authors. MIT license.

use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_config::deno_json::DesktopConfig;
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

  apply_desktop_config_to_flags(&mut desktop_flags, desktop_config);

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

/// Applies `deno.json`'s `desktop` config to the CLI flags. A `deno.json` field
/// only fills in a flag that was left unset — CLI flags always win. The webview
/// backend remains the final fallback via the `unwrap_or("webview")` call sites
/// that consume `desktop_flags.backend`.
fn apply_desktop_config_to_flags(
  desktop_flags: &mut DesktopFlags,
  desktop_config: DesktopConfig,
) {
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

    if let Some(deep_links) = app_config.deep_links
      && desktop_flags.deep_links.is_empty()
    {
      desktop_flags.deep_links = deep_links;
    }

    if let Some(allow_web_schemes) = app_config.allow_web_schemes {
      desktop_flags.allow_web_schemes = allow_web_schemes;
    }

    if let Some(permissions) = app_config.permissions
      && desktop_flags.macos_permissions.is_empty()
    {
      desktop_flags.macos_permissions = permissions;
    }

    if let Some(agent) = app_config.agent {
      desktop_flags.agent = agent;
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

  // Same for Linux `.deb` / `.rpm` package installers — strip the extension
  // for the intermediate compile/bundle step, then wrap the staged app dir in
  // the chosen package at the end. Both wrap the same tree produced by
  // `package_linux_app_dir`.
  let deb_output = desktop_flags
    .output
    .as_ref()
    .filter(|o| o.to_lowercase().ends_with(".deb"))
    .cloned();
  let rpm_output = desktop_flags
    .output
    .as_ref()
    .filter(|o| o.to_lowercase().ends_with(".rpm"))
    .cloned();
  if let Some(ref pkg) = deb_output.as_ref().or(rpm_output.as_ref()) {
    // `.deb`/`.rpm` wrap the staged Linux app dir, so the build must target
    // Linux. The package itself is assembled in pure Rust and cross-compiles
    // from any host — only the target OS matters. (Unlike `.dmg`, which is
    // gated on a macOS *host* because it shells out to hdiutil.)
    let targets_linux = match desktop_flags.target.as_deref() {
      Some(t) => t.contains("linux"),
      None => cfg!(target_os = "linux"),
    };
    if !targets_linux {
      bail!(
        "Building a {ext} requires a Linux target. Requested output: {pkg}. \
         Pass --target <linux-triple> (e.g. x86_64-unknown-linux-gnu) or build \
         on Linux.",
        ext = if deb_output.is_some() { ".deb" } else { ".rpm" },
      );
    }
    let stem = Path::new(pkg)
      .file_stem()
      .map(|s| s.to_string_lossy().into_owned())
      .unwrap_or_else(|| "App".to_string());
    let parent = Path::new(pkg)
      .parent()
      .filter(|p| !p.as_os_str().is_empty());
    desktop_flags.output = Some(match parent {
      Some(p) => p.join(&stem).to_string_lossy().into_owned(),
      None => stem,
    });
  }

  // Same for a Windows `.msi` installer — strip the extension for the
  // intermediate compile/bundle step, then wrap the staged Windows app dir in
  // an MSI at the end. The MSI is authored entirely in pure Rust (`msi` +
  // `cab`), so it cross-compiles from any host — only the *target* must be
  // Windows. (Unlike `.dmg`, which is gated on a macOS host because it shells
  // out to hdiutil.)
  let msi_output = desktop_flags
    .output
    .as_ref()
    .filter(|o| o.to_lowercase().ends_with(".msi"))
    .cloned();
  if let Some(ref msi) = msi_output {
    let targets_windows = match desktop_flags.target.as_deref() {
      Some(t) => t.contains("windows"),
      None => cfg!(target_os = "windows"),
    };
    if !targets_windows {
      bail!(
        "Building a .msi requires a Windows target. Requested output: {msi}. \
         Pass --target <windows-triple> (e.g. x86_64-pc-windows-msvc) or build \
         on Windows.",
      );
    }
    let stem = Path::new(msi)
      .file_stem()
      .map(|s| s.to_string_lossy().into_owned())
      .unwrap_or_else(|| "App".to_string());
    let parent = Path::new(msi)
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
      let use_framework_hmr =
        desktop_flags.hmr && detection.hmr_command.is_some();
      let entrypoint_code = if use_framework_hmr {
        NOOP_ENTRYPOINT.to_string()
      } else {
        detection.entrypoint_code.clone()
      };
      log::info!("Detected {} framework", detection.name);
      if !use_framework_hmr {
        // Run the framework's build step (e.g. `deno task build`) before its
        // build output (`dist`, `.next`, etc.) is added to the compile includes
        // below; otherwise the include points at a directory that doesn't exist
        // yet and the compile fails (#35535). Mirrors `deno compile .`.
        super::framework::run_build_command(detection, cwd)?;
      }
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
      // Add framework build output to includes. Skipped in HMR mode.
      if !use_framework_hmr {
        for inc in &detection.include_paths {
          if !desktop_flags.include.contains(inc) {
            desktop_flags.include.push(inc.clone());
          }
        }
      }
      Some(entrypoint_temp)
    } else {
      bail!(
        "Could not detect a supported framework in the current directory.\nSupported frameworks: Next.js, Astro, Fresh, Remix, React Router, SvelteKit, Nuxt, SolidStart, TanStack Start, Vite\nProvide an explicit entrypoint instead."
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
    app_name: None,
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
    bundle: false,
    minify: false,
    exclude_unused_npm: desktop_flags.exclude_unused_npm,
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

    // Optionally make the bundle self-extracting: the heavy payload is
    // compressed inside the shipped app and unpacked on first launch. This
    // shrinks the distributed artifact (the installed footprint is restored
    // on first run). Done before any .dmg/.deb/.AppImage wrapping so the
    // installer wraps the compact, self-extracting app.
    if let Some(format) = desktop_flags.compress.as_deref() {
      make_self_extracting(&bundle_path, format, &desktop_flags)?;
    }

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
    } else if let Some(deb) = deb_output.as_deref() {
      let deb_abs = cli_options.initial_cwd().join(deb);
      create_linux_deb(
        &bundle_path,
        &deb_abs,
        &desktop_flags,
        desktop_flags.target.as_deref(),
      )?;
      deb_abs
    } else if let Some(rpm) = rpm_output.as_deref() {
      let rpm_abs = cli_options.initial_cwd().join(rpm);
      create_linux_rpm(
        &bundle_path,
        &rpm_abs,
        &desktop_flags,
        desktop_flags.target.as_deref(),
      )?;
      rpm_abs
    } else if let Some(msi) = msi_output.as_deref() {
      let msi_abs = cli_options.initial_cwd().join(msi);
      create_windows_msi(
        &bundle_path,
        &msi_abs,
        &desktop_flags,
        desktop_flags.target.as_deref(),
      )?;
      msi_abs
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

/// Convert a packaged app bundle into a self-extracting one: the heavy payload
/// is compressed inside the shipped bundle and unpacked to a per-user data
/// directory on first launch, then the real app is exec'd from there.
///
/// This shrinks the distributed artifact (the installed footprint is restored
/// on first run, cached and reused on subsequent launches). The transform is
/// in place at `bundle_path`. `format` is `"xz"` (LZMA, smallest, decompressed
/// everywhere by libarchive `tar`) or `"zstd"` (faster, slightly larger).
fn make_self_extracting(
  bundle_path: &Path,
  format: &str,
  desktop_flags: &DesktopFlags,
) -> Result<(), AnyError> {
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
  match target_os {
    "macos" => make_self_extracting_macos(bundle_path, format, desktop_flags),
    "windows" => make_self_extracting_dir(bundle_path, format, true),
    _ => make_self_extracting_dir(bundle_path, format, false),
  }
}

/// Validate a deep-link URL scheme. Follows the RFC 3986 `scheme` grammar:
/// `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`. We additionally reject the
/// common reserved schemes (`http`, `https`, `file`, `ftp`, `ws`, `wss`) since
/// registering those as app handlers is almost never intended and would hijack
/// normal browsing.
///
/// `allow_web_schemes` (from `desktop.app.allowWebSchemes`) is an explicit
/// opt-in that lifts the reservation on `http`/`https` only — a genuine
/// default-browser-style utility legitimately needs to register those.
/// `file`/`ftp`/`ws`/`wss` remain reserved regardless.
fn validate_url_scheme(
  scheme: &str,
  allow_web_schemes: bool,
) -> Result<(), AnyError> {
  let reserved: &[&str] = if allow_web_schemes {
    &["file", "ftp", "ws", "wss"]
  } else {
    &["http", "https", "file", "ftp", "ws", "wss"]
  };
  let bail = |reason: &str| {
    Err(deno_core::anyhow::anyhow!(
      "Invalid deep-link scheme {scheme:?}: {reason}."
    ))
  };
  match scheme.chars().next() {
    None => return bail("scheme is empty"),
    Some(c) if !c.is_ascii_alphabetic() => {
      return bail("scheme must start with an ASCII letter");
    }
    _ => {}
  }
  if !scheme
    .chars()
    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
  {
    return bail("scheme may only contain letters, digits, '+', '-', and '.'");
  }
  if reserved.contains(&scheme) {
    return bail("scheme is reserved and cannot be used as a deep link");
  }
  Ok(())
}

/// Register the configured deep-link URL schemes with the OS-specific app
/// metadata so the system routes `<scheme>://...` links to this app.
///
/// First pass: this writes the declarative registration into the bundle
/// (macOS `CFBundleURLTypes`, Linux `.desktop` `MimeType` + `Exec %u`,
/// Windows `.reg`/`.bat` helper). Delivering the opened URL into the running
/// app (single-instance forwarding, the macOS `openURLs` Apple Event, and the
/// `open-url` JS event) is tracked separately in the issue.
fn register_deep_links(
  bundle_path: &Path,
  desktop_flags: &DesktopFlags,
) -> Result<(), AnyError> {
  let schemes: Vec<String> = desktop_flags
    .deep_links
    .iter()
    .map(|s| s.trim().to_ascii_lowercase())
    .filter(|s| !s.is_empty())
    .collect();
  if schemes.is_empty() {
    return Ok(());
  }
  for scheme in &schemes {
    validate_url_scheme(scheme, desktop_flags.allow_web_schemes)?;
  }

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
  match target_os {
    "macos" => register_deep_links_macos(bundle_path, &schemes)?,
    "windows" => register_deep_links_windows(bundle_path, &schemes)?,
    _ => register_deep_links_linux(bundle_path, &schemes)?,
  }

  log::info!(
    "{} {}",
    colors::green("Deep links"),
    schemes
      .iter()
      .map(|s| format!("{s}://"))
      .collect::<Vec<_>>()
      .join(", "),
  );
  Ok(())
}

/// macOS: add a single `CFBundleURLTypes` entry carrying every scheme to the
/// bundle `Info.plist`.
fn register_deep_links_macos(
  bundle_path: &Path,
  schemes: &[String],
) -> Result<(), AnyError> {
  let plist_path = bundle_path.join("Contents").join("Info.plist");
  let mut dict: plist::Dictionary = plist::from_file(&plist_path)
    .with_context(|| format!("failed to parse {}", plist_path.display()))?;

  let url_name = dict
    .get("CFBundleIdentifier")
    .and_then(|v| v.as_string())
    .unwrap_or("")
    .to_string();

  let mut url_type = plist::Dictionary::new();
  url_type.insert(
    "CFBundleURLName".to_string(),
    plist::Value::String(url_name),
  );
  url_type.insert(
    "CFBundleTypeRole".to_string(),
    plist::Value::String("Viewer".to_string()),
  );
  url_type.insert(
    "CFBundleURLSchemes".to_string(),
    plist::Value::Array(
      schemes.iter().cloned().map(plist::Value::String).collect(),
    ),
  );
  dict.insert(
    "CFBundleURLTypes".to_string(),
    plist::Value::Array(vec![plist::Value::Dictionary(url_type)]),
  );

  plist::to_file_xml(&plist_path, &dict)
    .with_context(|| format!("failed to write {}", plist_path.display()))?;
  Ok(())
}

/// Linux: add `x-scheme-handler/<scheme>` MIME types to the `.desktop` entry
/// and make sure `Exec=` forwards the opened URL via the `%u` field code.
fn register_deep_links_linux(
  bundle_path: &Path,
  schemes: &[String],
) -> Result<(), AnyError> {
  let desktop_file = std::fs::read_dir(bundle_path)?
    .filter_map(|e| e.ok().map(|e| e.path()))
    .find(|p| p.extension().is_some_and(|e| e == "desktop"))
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "no .desktop file found in {}",
        bundle_path.display()
      )
    })?;

  let contents = std::fs::read_to_string(&desktop_file)?;
  let mime = schemes
    .iter()
    .map(|s| format!("x-scheme-handler/{s};"))
    .collect::<String>();

  let mut out = String::with_capacity(contents.len() + mime.len() + 16);
  let mut wrote_mime = false;
  for line in contents.lines() {
    if let Some(rest) = line.strip_prefix("Exec=") {
      // The launcher must receive the URL as an argument, so ensure a `%u`
      // field code is present exactly once.
      if rest.contains("%u") || rest.contains("%U") {
        out.push_str(line);
      } else {
        out.push_str(&format!("Exec={} %u", rest.trim_end()));
      }
      out.push('\n');
    } else if let Some(rest) = line.strip_prefix("MimeType=") {
      // Merge into the existing MimeType list.
      out.push_str("MimeType=");
      out.push_str(rest.trim_end());
      if !rest.trim_end().ends_with(';') && !rest.trim_end().is_empty() {
        out.push(';');
      }
      out.push_str(&mime);
      out.push('\n');
      wrote_mime = true;
    } else {
      out.push_str(line);
      out.push('\n');
    }
  }
  if !wrote_mime {
    out.push_str(&format!("MimeType={mime}\n"));
  }

  std::fs::write(&desktop_file, out)?;
  Ok(())
}

/// Windows: there is no in-bundle declarative registration for protocol
/// handlers, so drop a `register-deep-links.bat` next to the launcher that
/// writes the `HKCU\Software\Classes\<scheme>` keys. An installer (or the
/// user) runs it once after install; the keys point back at the launcher in
/// its install location (`%~dp0`).
fn register_deep_links_windows(
  bundle_path: &Path,
  schemes: &[String],
) -> Result<(), AnyError> {
  // The launcher written by the Windows packaging step is `<app>.exe` (the
  // backend binary renamed to the bundle/app name).
  let launcher = bundle_path
    .file_name()
    .map(|n| format!("{}.exe", n.to_string_lossy()))
    .unwrap_or_else(|| "launcher.exe".to_string());

  let mut script = String::from("@echo off\r\nsetlocal\r\n");
  for scheme in schemes {
    script.push_str(&format!(
      "reg add \"HKCU\\Software\\Classes\\{scheme}\" /ve /d \"URL:{scheme}\" /f\r\n\
       reg add \"HKCU\\Software\\Classes\\{scheme}\" /v \"URL Protocol\" /d \"\" /f\r\n\
       reg add \"HKCU\\Software\\Classes\\{scheme}\\shell\\open\\command\" /ve /d \"\\\"%~dp0{launcher}\\\" \\\"%%1\\\"\" /f\r\n",
    ));
  }
  script.push_str("endlocal\r\n");

  std::fs::write(bundle_path.join("register-deep-links.bat"), script)?;
  Ok(())
}

/// Tar `parent/entry_name` (preserving symlinks and modes) into `dest_file`,
/// compressed with `format`. Returns `(uncompressed, compressed)` byte sizes.
fn write_tar_compressed(
  parent: &Path,
  entry_name: &str,
  dest_file: &Path,
  format: &str,
) -> Result<(u64, u64), AnyError> {
  use std::io::Write;
  let mut tar_buf = Vec::new();
  {
    let mut builder = tar::Builder::new(&mut tar_buf);
    builder.follow_symlinks(false);
    builder.append_dir_all(entry_name, parent.join(entry_name))?;
    builder.finish()?;
  }
  let raw_len = tar_buf.len() as u64;

  let out = std::fs::File::create(dest_file).with_context(|| {
    format!("failed to create payload {}", dest_file.display())
  })?;
  let out = std::io::BufWriter::new(out);
  match format {
    "xz" | "lzma" => {
      // Preset 9 ≈ `xz -9`; PRESET_EXTREME trades a lot of CPU for a few
      // percent, so stick to plain 9 for build-time sanity.
      let mut enc = liblzma::write::XzEncoder::new(out, 9);
      enc.write_all(&tar_buf)?;
      enc.finish()?.flush()?;
    }
    "zstd" => {
      let mut enc = zstd::stream::write::Encoder::new(out, 19)?;
      enc.write_all(&tar_buf)?;
      enc.finish()?.flush()?;
    }
    other => bail!("unknown --compress format '{other}' (use xz or zstd)"),
  }
  let comp_len = std::fs::metadata(dest_file).map(|m| m.len()).unwrap_or(0);
  Ok((raw_len, comp_len))
}

/// Short, stable cache key derived from the payload bytes — bumps the
/// extraction directory whenever the app contents change.
fn payload_hash(payload: &Path) -> Result<String, AnyError> {
  let bytes = std::fs::read(payload)?;
  let digest = sha2::Sha256::digest(&bytes);
  Ok(faster_hex::hex_string(&digest)[..16].to_string())
}

fn payload_ext(format: &str) -> &'static str {
  match format {
    "zstd" => "tar.zst",
    _ => "tar.xz",
  }
}

/// macOS: replace `MyApp.app` with a thin `.app` whose `Resources/` holds the
/// compressed real bundle. The `Contents/MacOS/<App>` launcher extracts it to
/// `~/Library/Application Support/<bundle-id>/<hash>/` on first run and execs
/// the real backend from there.
fn make_self_extracting_macos(
  bundle_path: &Path,
  format: &str,
  desktop_flags: &DesktopFlags,
) -> Result<(), AnyError> {
  let app_name = bundle_path
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| "App".to_string());
  validate_launcher_name(&app_name, "app name")?;

  let contents = bundle_path.join("Contents");
  let bundle_id =
    read_plist_string(&contents.join("Info.plist"), "CFBundleIdentifier")
      .unwrap_or_else(|| {
        format!("com.deno.desktop.{}", app_name.to_lowercase())
      });
  validate_bundle_identifier(&bundle_id)?;

  // Capture the bits we re-create in the thin bundle before moving the real
  // one out of the way.
  let info_plist = std::fs::read(contents.join("Info.plist"))?;
  let icon_src = contents.join("Resources").join("AppIcon.icns");
  let icon = icon_src
    .exists()
    .then(|| std::fs::read(&icon_src))
    .transpose()?;

  // Move the full, signed bundle into a staging dir as `<App>.app`.
  let parent = bundle_path.parent().unwrap_or_else(|| Path::new("."));
  let staging = tempfile::Builder::new()
    .prefix(".selfextract-")
    .tempdir_in(parent)?;
  let inner_name = format!("{app_name}.app");
  let inner = staging.path().join(&inner_name);
  std::fs::rename(bundle_path, &inner)?;

  // Build the thin bundle in place.
  let macos_dir = contents.join("MacOS");
  let resources_dir = contents.join("Resources");
  std::fs::create_dir_all(&macos_dir)?;
  std::fs::create_dir_all(&resources_dir)?;

  let payload_name = format!("payload.{}", payload_ext(format));
  let payload = resources_dir.join(&payload_name);
  let (raw, comp) =
    write_tar_compressed(staging.path(), &inner_name, &payload, format)?;
  let hash = payload_hash(&payload)?;

  let launcher = format!(
    "#!/bin/sh\n\
     set -e\n\
     DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
     DEST=\"$HOME/Library/Application Support/{bundle_id}/{hash}\"\n\
     APP=\"$DEST/{inner_name}\"\n\
     if [ ! -x \"$APP/Contents/MacOS/{app_name}\" ]; then\n\
     \u{20} mkdir -p \"$DEST\"\n\
     \u{20} tar -xf \"$DIR/../Resources/{payload_name}\" -C \"$DEST\"\n\
     fi\n\
     exec \"$APP/Contents/MacOS/{app_name}\" \"$@\"\n",
  );
  let launcher_path = macos_dir.join(&app_name);
  std::fs::write(&launcher_path, launcher)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
      &launcher_path,
      std::fs::Permissions::from_mode(0o755),
    )?;
  }

  std::fs::write(contents.join("Info.plist"), info_plist)?;
  if let Some(icon) = icon {
    std::fs::write(resources_dir.join("AppIcon.icns"), icon)?;
  }

  // Re-sign the thin bundle (the inner one keeps its own signature inside the
  // archive). Ad-hoc on a macOS host when no identity was given.
  let codesign_identity = desktop_flags.codesign_identity.as_deref().or(
    if cfg!(target_os = "macos") {
      Some("-")
    } else {
      None
    },
  );
  if let Some(identity) = codesign_identity {
    codesign_macos_bundle(bundle_path, identity)?;
  }

  log::info!(
    "{} {} ({} -> {}, {})",
    colors::green("Self-extract"),
    app_name,
    human_size(raw),
    human_size(comp),
    format,
  );
  Ok(())
}

/// Linux / Windows: replace the app directory with a thin one holding the
/// compressed payload plus a launcher that extracts to a per-user data dir
/// (`$XDG_DATA_HOME` / `%LOCALAPPDATA%`) on first run and execs the real app.
fn make_self_extracting_dir(
  bundle_path: &Path,
  format: &str,
  windows: bool,
) -> Result<(), AnyError> {
  let app_name = bundle_path
    .file_name()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| "App".to_string());
  validate_launcher_name(&app_name, "app name")?;
  let id = format!("com.deno.desktop.{}", app_name.to_lowercase());

  let parent = bundle_path.parent().unwrap_or_else(|| Path::new("."));
  let staging = tempfile::Builder::new()
    .prefix(".selfextract-")
    .tempdir_in(parent)?;
  let inner = staging.path().join(&app_name);
  std::fs::rename(bundle_path, &inner)?;

  std::fs::create_dir_all(bundle_path)?;
  let payload_name = format!("payload.{}", payload_ext(format));
  let payload = bundle_path.join(&payload_name);
  let (raw, comp) =
    write_tar_compressed(staging.path(), &app_name, &payload, format)?;
  let hash = payload_hash(&payload)?;

  if windows {
    let launcher = format!(
      "@echo off\r\n\
       setlocal\r\n\
       set \"DIR=%~dp0\"\r\n\
       set \"DEST=%LOCALAPPDATA%\\{id}\\{hash}\"\r\n\
       if not exist \"%DEST%\\{app_name}\\{app_name}.exe\" (\r\n\
       \u{20} mkdir \"%DEST%\" 2>nul\r\n\
       \u{20} tar -xf \"%DIR%{payload_name}\" -C \"%DEST%\"\r\n\
       )\r\n\
       \"%DEST%\\{app_name}\\{app_name}.exe\" %*\r\n",
    );
    std::fs::write(bundle_path.join(format!("{app_name}.bat")), launcher)?;
  } else {
    let launcher = format!(
      "#!/bin/sh\n\
       set -e\n\
       DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
       DEST=\"${{XDG_DATA_HOME:-$HOME/.local/share}}/{id}/{hash}\"\n\
       APP=\"$DEST/{app_name}\"\n\
       if [ ! -x \"$APP/{app_name}\" ]; then\n\
       \u{20} mkdir -p \"$DEST\"\n\
       \u{20} tar -xf \"$DIR/{payload_name}\" -C \"$DEST\"\n\
       fi\n\
       exec \"$APP/{app_name}\" \"$@\"\n",
    );
    let launcher_path = bundle_path.join(&app_name);
    std::fs::write(&launcher_path, launcher)?;
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      std::fs::set_permissions(
        &launcher_path,
        std::fs::Permissions::from_mode(0o755),
      )?;
    }
  }

  log::info!(
    "{} {} ({} -> {}, {})",
    colors::green("Self-extract"),
    app_name,
    human_size(raw),
    human_size(comp),
    format,
  );
  Ok(())
}

/// Format a byte count as a short human-readable size (e.g. `66.0M`).
fn human_size(bytes: u64) -> String {
  const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
  let mut size = bytes as f64;
  let mut unit = 0;
  while size >= 1024.0 && unit < UNITS.len() - 1 {
    size /= 1024.0;
    unit += 1;
  }
  if unit == 0 {
    format!("{}{}", bytes, UNITS[unit])
  } else {
    format!("{:.1}{}", size, UNITS[unit])
  }
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

/// Extract the local URL from a line of dev server output.
fn parse_dev_server_url(line: &str) -> Option<String> {
  // Vite prints `  ➜  Local:   http://localhost:5173/`
  regex::Regex::new(r"Local:\s+(https?://\S+)")
    .expect("regex to parse local url failed")
    .captures(line)
    .and_then(|c| c.get(1))
    .map(|m| m.as_str().to_owned())
}

/// Spawns a framework HMR dev server
async fn spawn_framework_dev_server(
  name: &str,
  cmd_args: &[String],
  cwd: &Path,
) -> Result<(String, tokio::process::Child), AnyError> {
  use tokio::io::AsyncBufReadExt;
  use tokio::io::BufReader;

  let mut child = tokio::process::Command::new(&cmd_args[0])
    .args(&cmd_args[1..])
    .current_dir(cwd)
    .stdout(std::process::Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .with_context(|| {
      format!("failed to spawn HMR dev server: {:?}", cmd_args)
    })?;

  let stdout = child.stdout.take().ok_or_else(|| {
    deno_core::anyhow::anyhow!("failed to capture HMR dev server stdout")
  })?;
  let mut lines = BufReader::new(stdout).lines();

  let url = tokio::time::timeout(std::time::Duration::from_secs(15), async {
    while let Ok(Some(line)) = lines.next_line().await {
      let url = parse_dev_server_url(&line);
      // Echo the dev server's own startup output (banner, URL, warnings) so
      // it isn't swallowed while we scan for the URL.
      log::info!("{line}");
      if let Some(url) = url {
        return Ok(url);
      }
    }
    deno_core::anyhow::bail!("dev server exited without printing a URL")
  })
  .await
  .map_err(|_| {
    deno_core::anyhow::anyhow!(
      "{name} dev server with HMR did not start within 15s"
    )
  })??;

  // Keep forwarding the dev server's stdout so its HMR/compile logs stay
  // visible after startup instead of being silently swallowed. The task ends
  // on its own once the child is killed (on drop) and the pipe closes.
  tokio::spawn(async move {
    while let Ok(Some(line)) = lines.next_line().await {
      log::info!("{line}");
    }
  });

  Ok((url, child))
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
      "{} {}desktop app with HMR (watching {})",
      colors::green("Running"),
      framework
        .map(|f| format!("{} ", f.name))
        .unwrap_or_default(),
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

  let _dev_server_child = if desktop_flags.hmr
    && let Some(fw) = framework
    && let Some(dev_cmd) = &fw.hmr_command
  {
    let (dev_url, child) =
      spawn_framework_dev_server(fw.name, dev_cmd, &source_abs).await?;
    log::info!(
      "{} {} HMR dev server at {}",
      colors::green("Running"),
      fw.name,
      dev_url,
    );
    cmd.env("DENO_DESKTOP_DEV_URL", &dev_url);
    Some(child)
  } else {
    None
  };

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

/// Marker file written into every generated desktop app directory/bundle so a
/// later build can recognize its own previous output and clear it, while never
/// touching unrelated user data that happens to share the inferred app name.
const APP_DIR_MARKER: &str = ".deno-desktop-app";

/// Used for `deno desktop --hmr` when a framework runs its own HMR server.
/// The compiled app has nothing to serve in this case.
const NOOP_ENTRYPOINT: &str =
  "// @ts-nocheck\nawait new Promise<void>(() => {});\n";

/// Prepare `app_dir` to receive a freshly built bundle.
///
/// The app name is inferred from the entrypoint (or, for generic names like
/// `main.ts`, the project directory), so the output path can collide with an
/// existing user directory of the same name (e.g. `helloworld/helloworld`).
/// Blindly `remove_dir_all`-ing that path silently destroys the user's data
/// (issue #35510). Instead, only remove a directory we previously created —
/// identified by `APP_DIR_MARKER` — or one that is empty. Anything else is
/// treated as user data and we bail with instructions rather than delete it.
fn reserve_app_dir(app_dir: &Path) -> Result<(), AnyError> {
  let meta = match std::fs::symlink_metadata(app_dir) {
    // Nothing there yet (or unreadable) — let the build create it.
    Err(_) => return Ok(()),
    Ok(meta) => meta,
  };

  if !meta.is_dir() {
    bail!(
      "Refusing to overwrite '{}': a file with that name already exists. \
       Pass --output to choose a different name.",
      app_dir.display()
    );
  }

  // A bundle we generated carries the marker (at the directory root for
  // Linux/Windows, or under `Contents/Resources` for a macOS `.app`).
  let is_ours = app_dir.join(APP_DIR_MARKER).exists()
    || app_dir
      .join("Contents")
      .join("Resources")
      .join(APP_DIR_MARKER)
      .exists();
  let is_empty = std::fs::read_dir(app_dir)
    .map(|mut entries| entries.next().is_none())
    .unwrap_or(false);

  if !is_ours && !is_empty {
    bail!(
      "Refusing to delete existing directory '{}': it was not created by \
       `deno desktop`. The app name was inferred from the entrypoint or the \
       project directory and collided with this directory. Pass --output to \
       choose a different name, or remove the directory yourself if it is a \
       leftover from an older build.",
      app_dir.display()
    );
  }

  std::fs::remove_dir_all(app_dir).with_context(|| {
    format!("failed to clear app directory {}", app_dir.display())
  })?;
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
///   AppName.exe         (LAUFEY backend binary, renamed to the app name)
///   libcef.dll, ...     (CEF support files, if any)
///   AppName.dll         (compiled Deno runtime + user code)
///   AppIcon.ico         (optional)
/// ```
///
/// The backend binary is renamed to `AppName.exe` so it auto-loads the
/// co-located `AppName.dll` runtime — no `.bat` launcher is needed.
async fn package_windows_app_dir(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  laufey_resolver: &LaufeyBackendResolver,
) -> Result<PathBuf, AnyError> {
  let parts = dylib_parts(dylib_path)?;
  let app_name = parts.app_name;
  let app_dir = parts.parent.join(&app_name);

  let backend = desktop_flags.backend.as_deref().unwrap_or("webview");
  let target = laufey_target_for(desktop_flags);
  let laufey_binary = laufey_resolver.find_binary(backend, target).await?;
  let laufey_dir = laufey_resolver.find_binary_dir(backend, target).await?;
  let laufey_binary_name = laufey_binary
    .file_name()
    .unwrap()
    .to_string_lossy()
    .to_string();

  reserve_app_dir(&app_dir)?;

  // Copy LAUFEY backend directory (binary + CEF support files) as the shell.
  crate::tools::compile::copy_dir_all(&laufey_dir, &app_dir)?;
  std::fs::write(app_dir.join(APP_DIR_MARKER), b"")?;

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

  // Rename the LAUFEY backend binary to the app name (`<app>.exe`) so it sits
  // next to `<app>.dll` and auto-loads it: laufey's LaufeyFindColocatedRuntime
  // resolves a runtime library whose base name matches the executable's, in the
  // executable's own directory. A bare double-click of `<app>.exe` therefore
  // "just works" with no `.bat` wrapper and no `--runtime` argument.
  //
  // This also fixes runtime loading from paths containing a space (e.g. the
  // default MSI target `C:\Program Files\`): laufey resolves the co-located
  // runtime from its own module path rather than a `--runtime` string, avoiding
  // the historical `ERROR_MOD_NOT_FOUND` (126) failure on such paths.
  validate_launcher_name(&app_name, "app name")?;
  let launcher_path = app_dir.join(format!("{}.exe", app_name));
  let staged_backend = app_dir.join(&laufey_binary_name);
  if staged_backend != launcher_path {
    std::fs::rename(&staged_backend, &launcher_path)?;
  }

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

  // Drop the deep-link registration script next to the launcher.
  register_deep_links(&app_dir, desktop_flags)?;

  // Remove the standalone dylib (it's now inside the app dir).
  let _ = std::fs::remove_file(dylib_path);

  Ok(app_dir)
}

/// Create a Linux app directory from the compiled desktop dylib.
///
/// Directory structure:
/// ```text
/// AppName/
///   AppName             (LAUFEY backend binary, renamed to the app name)
///   libcef.so, ...      (CEF support files, if any)
///   AppName.so          (compiled Deno runtime + user code)
///   AppIcon.png         (optional)
/// ```
///
/// The backend binary is renamed to `AppName` so it auto-loads the co-located
/// `AppName.so` runtime — no launcher shell script is needed.
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

  let backend = desktop_flags.backend.as_deref().unwrap_or("webview");
  let target = laufey_target_for(desktop_flags);
  let laufey_binary = laufey_resolver.find_binary(backend, target).await?;
  let laufey_dir = laufey_resolver.find_binary_dir(backend, target).await?;
  let laufey_binary_name = laufey_binary
    .file_name()
    .unwrap()
    .to_string_lossy()
    .to_string();

  reserve_app_dir(&app_dir)?;

  // Copy LAUFEY backend directory (binary + CEF support files) as the shell.
  crate::tools::compile::copy_dir_all(&laufey_dir, &app_dir)?;
  std::fs::write(app_dir.join(APP_DIR_MARKER), b"")?;

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

  // Copy the compiled dylib as `<app>.so` so the renamed backend binary
  // auto-loads it: laufey's LaufeyFindColocatedRuntime resolves `<exe-base>.so`
  // next to the binary. It reads the real path via `/proc/self/exe`, which
  // follows the `/usr/bin/<pkg>` -> `/usr/lib/<pkg>/<app>` symlink that
  // `.deb`/`.rpm` install (issue #35623), so no wrapper script is needed.
  validate_launcher_name(&app_name, "app name")?;
  let dest_dylib = app_dir.join(format!("{}.so", app_name));
  std::fs::copy(dylib_path, &dest_dylib)?;

  // Rename the LAUFEY backend binary to the app name so `<app>` is the launcher
  // the user runs directly — no `--runtime` argument and no shell wrapper.
  let launcher_path = app_dir.join(&app_name);
  let staged_backend = app_dir.join(&laufey_binary_name);
  if staged_backend != launcher_path {
    std::fs::rename(&staged_backend, &launcher_path)?;
  }
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

  // Merge any deep-link schemes into the `.desktop` entry written above.
  register_deep_links(&app_dir, desktop_flags)?;

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
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        let dest_path = dest.join(&entry_path);
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
  // The bare build-tree binaries carry the host executable suffix — `.exe` on
  // Windows, empty elsewhere — so e.g. `cargo build` on Windows emits
  // `laufey_winit.exe`. The macOS `.app` bundle paths never apply on Windows,
  // so leaving them unsuffixed is fine.
  let exe =
    |rel: &str| laufey.join(format!("{rel}{}", std::env::consts::EXE_SUFFIX));
  let candidates: Vec<PathBuf> = match backend {
    "cef" => vec![
      laufey.join("result-cef/Applications/laufey.app/Contents/MacOS/laufey"),
      laufey.join("result/Applications/laufey.app/Contents/MacOS/laufey"),
      laufey.join("cef/build/Release/laufey.app/Contents/MacOS/laufey"),
      laufey.join("cef/build/laufey.app/Contents/MacOS/laufey"),
      exe("cef/build/Release/laufey"),
      exe("cef/build/laufey"),
    ],
    "raw" => vec![
      exe("target/release/laufey_winit"),
      exe("target/debug/laufey_winit"),
    ],
    _ => vec![
      laufey.join(
        "result-1/Applications/laufey_webview.app/Contents/MacOS/laufey_webview",
      ),
      laufey
        .join("result/Applications/laufey_webview.app/Contents/MacOS/laufey_webview"),
      laufey.join("webview/build/laufey_webview.app/Contents/MacOS/laufey_webview"),
      exe("webview/build/laufey_webview"),
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
    "raw" => return None,
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

/// The runtime dylib filename each macOS LAUFEY backend resolves when the
/// backend binary is the bundle's CFBundleExecutable (i.e. no `--runtime`
/// argument is passed). The two macOS backends use different conventions:
/// - `webview` searches a hardcoded `libruntime.dylib` via [NSBundle mainBundle]
///   (laufey `webview/src/main_mac.mm`).
/// - `cef` derives `<backend-executable-basename>.dylib` next to the binary
///   (laufey `LaufeyFindColocatedRuntime`, `cef/src/runtime_loader.cc`).
fn macos_runtime_dylib_name(backend: &str, laufey_exe_stem: &str) -> String {
  if backend == "cef" {
    format!("{laufey_exe_stem}.dylib")
  } else {
    "libruntime.dylib".to_string()
  }
}

/// Create a macOS .app bundle from the compiled desktop dylib.
///
/// Bundle structure:
/// ```text
/// AppName.app/
///   Contents/
///     Info.plist          (CFBundleExecutable = the LAUFEY backend binary)
///     MacOS/
///       laufey_webview    (LAUFEY backend binary; the bundle executable)
///       libruntime.dylib  (compiled Deno runtime + user code; the `cef`
///                          backend uses `<backend-executable>.dylib` instead)
///     Resources/
///       AppIcon.icns      (optional)
/// ```
async fn package_macos_app_bundle(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
  laufey_resolver: &LaufeyBackendResolver,
) -> Result<PathBuf, AnyError> {
  let parts = dylib_parts(dylib_path)?;
  let app_name = parts.app_name.clone();
  // app_name (from the --output stem) flows unescaped into the Info.plist XML
  // and the synthesized bundle id, so reject anything outside the safe charset
  // (the same guard the removed shell launcher applied) instead of emitting a
  // malformed plist that silently fails to launch.
  validate_launcher_name(&app_name, "app name")?;
  let app_bundle = parts.parent.join(format!("{}.app", app_name));

  // Find the LAUFEY backend .app and its main executable.
  let backend = desktop_flags.backend.as_deref().unwrap_or("webview");
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

  // Remove an existing bundle we previously built; never delete unrelated
  // user data that collided with the inferred `<name>.app` (issue #35510).
  reserve_app_dir(&app_bundle)?;

  // Copy the entire LAUFEY .app as the shell (CEF needs Frameworks/, Resources/, etc.).
  crate::tools::compile::copy_dir_all(&laufey_app, &app_bundle)?;

  let contents_dir = app_bundle.join("Contents");
  let macos_dir = contents_dir.join("MacOS");
  let resources_dir = contents_dir.join("Resources");
  std::fs::create_dir_all(&resources_dir)?;
  // Marker lives under Resources/ (not the bundle root) so it is sealed as a
  // normal resource when the bundle is codesigned below.
  std::fs::write(resources_dir.join(APP_DIR_MARKER), b"")?;

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

  // Copy the compiled dylib under the name the active backend resolves without
  // a `--runtime` argument (see `macos_runtime_dylib_name`). We deliberately do
  // NOT write a shell-script launcher that execs the backend with `--runtime`:
  // under LaunchServices (open / Finder / double-click) that `exec` breaks the
  // app's foreground-GUI registration, so an `NSStatusItem` (Deno.Tray) is
  // created but never attaches to the menu bar. Instead the backend binary is
  // the bundle's CFBundleExecutable directly (see Info.plist below), so the
  // process LaunchServices launches IS the GUI process. See denoland/deno#35619.
  let dest_dylib =
    macos_dir.join(macos_runtime_dylib_name(backend, &laufey_exe_stem));
  std::fs::copy(dylib_path, &dest_dylib)?;

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

  // Generate Info.plist. The backend binary is the CFBundleExecutable so
  // there is no shell-script `exec` between LaunchServices and the GUI
  // process (which would break tray registration — see above).
  let info_plist = render_macos_info_plist(
    &app_name,
    &bundle_id,
    &laufey_executable_name,
    desktop_flags.icon.is_some(),
    &desktop_flags.macos_permissions,
    desktop_flags.agent,
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
  //
  // Register any deep-link URL schemes before signing: codesign seals the
  // bundle contents, so mutating `Info.plist` afterwards would invalidate
  // the signature and the app would be rejected on launch.
  register_deep_links(&app_bundle, desktop_flags)?;

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

/// Map a `desktop.app.permissions` entry to the `NS…UsageDescription` key(s)
/// and human-readable purpose it declares in the Info.plist. Unknown entries
/// are ignored (returns an empty slice) so a typo never silently drops a real
/// permission without a build-time signal elsewhere.
fn macos_permission_usage_keys(
  permission: &str,
) -> &'static [(&'static str, &'static str)] {
  match permission.trim().to_ascii_lowercase().as_str() {
    "microphone" | "mic" => {
      &[("NSMicrophoneUsageDescription", "the microphone")]
    }
    "camera" => &[("NSCameraUsageDescription", "the camera")],
    "audiocapture" | "audio-capture" | "audio_capture" => {
      &[("NSAudioCaptureUsageDescription", "audio capture")]
    }
    "bluetooth" => &[
      ("NSBluetoothAlwaysUsageDescription", "Bluetooth"),
      ("NSBluetoothPeripheralUsageDescription", "Bluetooth"),
    ],
    _ => &[],
  }
}

// Keep the generated bundle metadata aligned with laufey's macOS app plist.
// The TCC usage-description keys (microphone/camera/audio-capture/bluetooth)
// are emitted only for the permissions the app declares via
// `desktop.app.permissions`; a plain browser/menu-bar utility ships none of
// them. `agent` (from `desktop.app.agent`) emits `LSUIElement` so the app runs
// as a menu-bar accessory with no Dock icon.
fn render_macos_info_plist(
  app_name: &str,
  bundle_id: &str,
  executable_name: &str,
  has_icon: bool,
  permissions: &[String],
  agent: bool,
) -> String {
  // Emit each declared usage-description key at most once, in a stable order.
  let mut usage_descriptions = String::new();
  let mut seen: Vec<&'static str> = Vec::new();
  for permission in permissions {
    for (key, purpose) in macos_permission_usage_keys(permission) {
      if seen.contains(key) {
        continue;
      }
      seen.push(key);
      usage_descriptions.push_str(&format!(
        "  <key>{key}</key>\n  <string>{app_name} requires access to {purpose}</string>\n"
      ));
    }
  }

  let lsuielement = if agent {
    "  <key>LSUIElement</key>\n  <true/>\n"
  } else {
    ""
  };

  format!(
    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>{executable_name}</string>
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
{lsuielement}  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSPrincipalClass</key>
  <string>LaufeyApplication</string>
  <key>NSSupportsAutomaticGraphicsSwitching</key>
  <true/>
  <key>NSAppTransportSecurity</key>
  <dict>
    <key>NSAllowsLocalNetworking</key>
    <true/>
  </dict>
{usage_descriptions}</dict>
</plist>
"#,
    app_name = app_name,
    bundle_id = bundle_id,
    executable_name = executable_name,
    icon_file = if has_icon { "AppIcon" } else { "" },
    lsuielement = lsuielement,
    usage_descriptions = usage_descriptions,
  )
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
  let compressor = backhand::FilesystemCompressor::new(
    backhand::compression::Compressor::Zstd,
    None,
  )
  .context("Failed to configure zstd SquashFS compressor")?;
  writer.set_compressor(compressor);

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

/// CEF/Chromium shared-library runtime dependencies, as
/// `(soname, debian package)` pairs.
///
/// The Debian package names go into the `.deb` `Depends` field. The sonames go
/// into the `.rpm` `Requires` field: every RPM auto-`Provides` the sonames of
/// the shared libraries it ships, so a soname `Requires` resolves correctly on
/// Fedora, openSUSE, etc. without hard-coding each distro's divergent package
/// names. Too loose a list crashes the app on launch with a missing `.so`; too
/// strict blocks install on otherwise-fine systems — this is the curated middle
/// covering CEF's GTK/X11/NSS/audio needs.
const CEF_RUNTIME_DEPS: &[(&str, &str)] = &[
  ("libgtk-3.so.0", "libgtk-3-0"),
  ("libnss3.so", "libnss3"),
  ("libasound.so.2", "libasound2"),
  ("libX11.so.6", "libx11-6"),
  ("libXcomposite.so.1", "libxcomposite1"),
  ("libXdamage.so.1", "libxdamage1"),
  ("libXext.so.6", "libxext6"),
  ("libXfixes.so.3", "libxfixes3"),
  ("libXrandr.so.2", "libxrandr2"),
  ("libgbm.so.1", "libgbm1"),
  ("libxkbcommon.so.0", "libxkbcommon0"),
  ("libpango-1.0.so.0", "libpango-1.0-0"),
  ("libcairo.so.2", "libcairo2"),
  ("libatk-1.0.so.0", "libatk1.0-0"),
  ("libdbus-1.so.3", "libdbus-1-3"),
  ("libexpat.so.1", "libexpat1"),
  ("libxcb.so.1", "libxcb1"),
  ("libdrm.so.2", "libdrm2"),
];

/// Default package version when no version is configured. Matches the macOS
/// bundle's hard-coded `CFBundleVersion`.
const LINUX_PACKAGE_VERSION: &str = "1.0.0";

/// Metadata shared by the `.deb` and `.rpm` builders, derived from the staged
/// app dir name and the desktop flags. Avoids new config fields — the package
/// name comes from the app dir, the identifier from `--identifier` (or the same
/// synthetic `com.deno.desktop.<slug>` default the `.desktop` writer uses).
struct LinuxPackageMeta {
  /// Sanitized lowercase package name (Debian rules: `[a-z0-9][a-z0-9+.-]+`).
  /// Used as the package name, the `/usr/bin` symlink, and the icon/`.desktop`
  /// basenames.
  package: String,
  /// Human display name (the staged app dir's name), used for `Name=` in the
  /// `.desktop` entry and the package summary.
  app_name: String,
  version: String,
  maintainer: String,
  summary: String,
  /// Reverse-DNS identifier for the `.desktop` `StartupWMClass`.
  identifier: String,
}

/// Sanitize an app name into a Debian-style package name: lowercase, only
/// `[a-z0-9+.-]`, and a leading alphanumeric (Debian forbids a leading `+`/`-`/
/// `.`). Falls back to `app` when nothing usable remains.
fn debian_package_name(app_name: &str) -> String {
  let mut out = String::with_capacity(app_name.len());
  for c in app_name.to_lowercase().chars() {
    if c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-') {
      out.push(c);
    } else {
      out.push('-');
    }
  }
  let trimmed = out.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
  let trimmed = trimmed.trim_end_matches('-');
  if trimmed.len() < 2 {
    "app".to_string()
  } else {
    trimmed.to_string()
  }
}

fn linux_package_meta(
  app_dir: &Path,
  desktop_flags: &DesktopFlags,
) -> LinuxPackageMeta {
  let app_name = app_dir
    .file_name()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| "App".to_string());
  let package = debian_package_name(&app_name);
  let identifier = desktop_flags
    .identifier
    .clone()
    .unwrap_or_else(|| format!("com.deno.desktop.{package}"));
  LinuxPackageMeta {
    summary: format!("{app_name} desktop application"),
    maintainer: format!("{app_name} <noreply@deno.com>"),
    version: LINUX_PACKAGE_VERSION.to_string(),
    package,
    app_name,
    identifier,
  }
}

/// Map a target triple (or the host arch when `target` is None) to a Debian
/// architecture name. Debian arch names differ from the triple's leading
/// component (`x86_64` → `amd64`, `aarch64` → `arm64`).
fn debian_arch_for_target(
  target: Option<&str>,
) -> Result<&'static str, AnyError> {
  let arch = target
    .and_then(|t| t.split('-').next())
    .unwrap_or(std::env::consts::ARCH);
  match arch {
    "x86_64" => Ok("amd64"),
    "aarch64" => Ok("arm64"),
    other => bail!(
      "No Debian architecture mapping for arch '{other}'; supported: x86_64, aarch64"
    ),
  }
}

/// Map a target triple (or the host arch) to an RPM architecture name. RPM
/// keeps the triple's arch names (`x86_64`, `aarch64`).
fn rpm_arch_for_target(target: Option<&str>) -> Result<&'static str, AnyError> {
  let arch = target
    .and_then(|t| t.split('-').next())
    .unwrap_or(std::env::consts::ARCH);
  match arch {
    "x86_64" => Ok("x86_64"),
    "aarch64" => Ok("aarch64"),
    other => bail!(
      "No RPM architecture mapping for arch '{other}'; supported: x86_64, aarch64"
    ),
  }
}

/// `.desktop` entry installed at `/usr/share/applications/<pkg>.desktop`.
///
/// Unlike the in-app-dir `.desktop` (whose `Exec`/`Icon` are relative), this
/// one points `Exec` at the package name (resolved via PATH from the
/// `/usr/bin/<pkg>` symlink) and `Icon` at the installed hicolor icon name.
fn system_desktop_entry(meta: &LinuxPackageMeta) -> String {
  format!(
    "[Desktop Entry]\n\
     Type=Application\n\
     Name={app_name}\n\
     Exec={package}\n\
     Icon={package}\n\
     StartupWMClass={identifier}\n\
     Categories=Utility;\n",
    app_name = meta.app_name,
    package = meta.package,
    identifier = meta.identifier,
  )
}

/// Wrap a Linux app directory in a Debian `.deb` package.
///
/// A `.deb` is an `ar` archive of three members: `debian-binary` (`2.0\n`),
/// `control.tar.gz` (metadata) and `data.tar.gz` (the install tree at absolute
/// paths). Built entirely in Rust (`tar` + `flate2`, plus a hand-written `ar`
/// header) so it cross-compiles from any build host. Install layout:
///
/// ```text
/// /usr/lib/<pkg>/            ← staged app dir contents
/// /usr/bin/<pkg>             ← symlink → ../lib/<pkg>/<launcher>
/// /usr/share/applications/<pkg>.desktop
/// /usr/share/icons/hicolor/512x512/apps/<pkg>.png
/// ```
fn create_linux_deb(
  app_dir: &Path,
  deb_path: &Path,
  desktop_flags: &DesktopFlags,
  target: Option<&str>,
) -> Result<(), AnyError> {
  let meta = linux_package_meta(app_dir, desktop_flags);
  let arch = debian_arch_for_target(target)?;

  let data_tar_gz = build_deb_data_tar(app_dir, &meta)?;
  let installed_size_kib = data_tar_gz.installed_size_kib;

  let depends = CEF_RUNTIME_DEPS
    .iter()
    .map(|(_, pkg)| *pkg)
    .collect::<Vec<_>>()
    .join(", ");
  let control = format!(
    "Package: {package}\n\
     Version: {version}\n\
     Architecture: {arch}\n\
     Maintainer: {maintainer}\n\
     Installed-Size: {size}\n\
     Depends: {depends}\n\
     Section: utils\n\
     Priority: optional\n\
     Description: {summary}\n",
    package = meta.package,
    version = meta.version,
    maintainer = meta.maintainer,
    size = installed_size_kib,
    summary = meta.summary,
  );
  let control_tar_gz = build_deb_control_tar(&control)?;

  // Assemble the ar archive: global header then the three members in the
  // order dpkg expects (debian-binary, control, data).
  let mut out = Vec::new();
  out.extend_from_slice(b"!<arch>\n");
  ar_append_member(&mut out, "debian-binary", b"2.0\n");
  ar_append_member(&mut out, "control.tar.gz", &control_tar_gz);
  ar_append_member(&mut out, "data.tar.gz", &data_tar_gz.bytes);

  if let Some(parent) = deb_path.parent()
    && !parent.as_os_str().is_empty()
  {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(deb_path, &out).with_context(|| {
    format!("Failed to write .deb at {}", deb_path.display())
  })?;
  Ok(())
}

/// Append one member to an `ar` archive. The 60-byte header is the common
/// `ar` format dpkg uses: name (space-padded), mtime, uid/gid, mode, decimal
/// size, then the `` `\n `` magic. Member data is padded to an even length
/// with a trailing newline (per the `ar` spec).
fn ar_append_member(out: &mut Vec<u8>, name: &str, data: &[u8]) {
  let mut header = [b' '; 60];
  let name_bytes = name.as_bytes();
  header[..name_bytes.len()].copy_from_slice(name_bytes);
  // mtime (16..28), uid (28..34), gid (34..40)
  header[16..16 + 1].copy_from_slice(b"0");
  header[28..28 + 1].copy_from_slice(b"0");
  header[34..34 + 1].copy_from_slice(b"0");
  // mode (40..48), octal
  let mode = b"100644";
  header[40..40 + mode.len()].copy_from_slice(mode);
  // size (48..58), decimal
  let size = data.len().to_string();
  header[48..48 + size.len()].copy_from_slice(size.as_bytes());
  // magic (58..60)
  header[58] = b'`';
  header[59] = b'\n';
  out.extend_from_slice(&header);
  out.extend_from_slice(data);
  if data.len() % 2 == 1 {
    out.push(b'\n');
  }
}

struct DebDataTar {
  bytes: Vec<u8>,
  /// Sum of regular-file sizes rounded up to KiB, for the control file's
  /// `Installed-Size:` field.
  installed_size_kib: u64,
}

/// Build the `data.tar.gz`: the install tree at absolute (`./`-prefixed) paths.
fn build_deb_data_tar(
  app_dir: &Path,
  meta: &LinuxPackageMeta,
) -> Result<DebDataTar, AnyError> {
  use flate2::Compression;
  use flate2::write::GzEncoder;

  let mut tar_buf: Vec<u8> = Vec::new();
  let mut installed_size: u64 = 0;
  {
    let mut builder = tar::Builder::new(&mut tar_buf);

    let push_dir = |b: &mut tar::Builder<&mut Vec<u8>>,
                    path: &str|
     -> Result<(), AnyError> {
      let mut h = tar::Header::new_gnu();
      h.set_entry_type(tar::EntryType::Directory);
      h.set_mode(0o755);
      h.set_uid(0);
      h.set_gid(0);
      h.set_mtime(0);
      h.set_size(0);
      h.set_cksum();
      b.append_data(&mut h, path, std::io::empty())?;
      Ok(())
    };

    // Intermediate directories the package owns or shares. The `./` prefix is
    // conventional for `data.tar`; the `tar` crate strips it (it skips
    // `CurDir` components), leaving root-relative `usr/...` entries, which dpkg
    // installs to `/usr/...` identically.
    for dir in [
      "./usr/",
      "./usr/bin/",
      "./usr/lib/",
      &format!("./usr/lib/{}/", meta.package),
      "./usr/share/",
      "./usr/share/applications/",
      "./usr/share/icons/",
      "./usr/share/icons/hicolor/",
      "./usr/share/icons/hicolor/512x512/",
      "./usr/share/icons/hicolor/512x512/apps/",
    ] {
      push_dir(&mut builder, dir)?;
    }

    // Copy the staged app dir under /usr/lib/<pkg>/.
    let lib_prefix = format!("./usr/lib/{}", meta.package);
    let mut stack = vec![app_dir.to_path_buf()];
    let mut files: Vec<PathBuf> = Vec::new();
    while let Some(dir) = stack.pop() {
      let mut entries: Vec<_> =
        std::fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
      entries.sort_by_key(|e| e.file_name());
      for entry in entries {
        files.push(entry.path());
        if std::fs::symlink_metadata(entry.path())?.is_dir() {
          stack.push(entry.path());
        }
      }
    }
    files.sort();
    for path in files {
      let rel = path.strip_prefix(app_dir)?;
      let arc_path = format!("{}/{}", lib_prefix, rel.to_string_lossy());
      let meta_fs = std::fs::symlink_metadata(&path)?;
      let mode = unix_mode_of(&meta_fs) as u32;
      let mut h = tar::Header::new_gnu();
      h.set_uid(0);
      h.set_gid(0);
      h.set_mtime(0);
      if meta_fs.is_dir() {
        h.set_entry_type(tar::EntryType::Directory);
        h.set_mode(if mode == 0 { 0o755 } else { mode });
        h.set_size(0);
        h.set_cksum();
        builder.append_data(
          &mut h,
          format!("{arc_path}/"),
          std::io::empty(),
        )?;
      } else if meta_fs.file_type().is_symlink() {
        let link = std::fs::read_link(&path)?;
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_mode(0o777);
        h.set_size(0);
        h.set_link_name(&link)?;
        h.set_cksum();
        builder.append_data(&mut h, &arc_path, std::io::empty())?;
      } else {
        let data = std::fs::read(&path)?;
        installed_size += data.len() as u64;
        h.set_entry_type(tar::EntryType::Regular);
        h.set_mode(if mode == 0 { 0o644 } else { mode });
        h.set_size(data.len() as u64);
        h.set_cksum();
        builder.append_data(&mut h, &arc_path, &data[..])?;
      }
    }

    // /usr/bin/<pkg> → ../lib/<pkg>/<launcher>
    {
      let mut h = tar::Header::new_gnu();
      h.set_entry_type(tar::EntryType::Symlink);
      h.set_mode(0o777);
      h.set_uid(0);
      h.set_gid(0);
      h.set_mtime(0);
      h.set_size(0);
      h.set_link_name(format!("../lib/{}/{}", meta.package, meta.app_name))?;
      h.set_cksum();
      builder.append_data(
        &mut h,
        format!("./usr/bin/{}", meta.package),
        std::io::empty(),
      )?;
    }

    // /usr/share/applications/<pkg>.desktop
    {
      let desktop = system_desktop_entry(meta).into_bytes();
      installed_size += desktop.len() as u64;
      let mut h = tar::Header::new_gnu();
      h.set_entry_type(tar::EntryType::Regular);
      h.set_mode(0o644);
      h.set_uid(0);
      h.set_gid(0);
      h.set_mtime(0);
      h.set_size(desktop.len() as u64);
      h.set_cksum();
      builder.append_data(
        &mut h,
        format!("./usr/share/applications/{}.desktop", meta.package),
        &desktop[..],
      )?;
    }

    // /usr/share/icons/.../<pkg>.png (if the app dir carries an icon)
    let icon_src = app_dir.join("AppIcon.png");
    if icon_src.exists() {
      let data = std::fs::read(&icon_src)?;
      installed_size += data.len() as u64;
      let mut h = tar::Header::new_gnu();
      h.set_entry_type(tar::EntryType::Regular);
      h.set_mode(0o644);
      h.set_uid(0);
      h.set_gid(0);
      h.set_mtime(0);
      h.set_size(data.len() as u64);
      h.set_cksum();
      builder.append_data(
        &mut h,
        format!(
          "./usr/share/icons/hicolor/512x512/apps/{}.png",
          meta.package
        ),
        &data[..],
      )?;
    }

    builder.finish()?;
  }

  let mut gz = Vec::new();
  let mut enc = GzEncoder::new(&mut gz, Compression::default());
  std::io::Write::write_all(&mut enc, &tar_buf)?;
  enc.finish()?;
  Ok(DebDataTar {
    bytes: gz,
    installed_size_kib: installed_size.div_ceil(1024).max(1),
  })
}

/// Build the `control.tar.gz` carrying the single `./control` file.
fn build_deb_control_tar(control: &str) -> Result<Vec<u8>, AnyError> {
  use flate2::Compression;
  use flate2::write::GzEncoder;

  let mut tar_buf: Vec<u8> = Vec::new();
  {
    let mut builder = tar::Builder::new(&mut tar_buf);
    let body = control.as_bytes();
    let mut h = tar::Header::new_gnu();
    h.set_entry_type(tar::EntryType::Regular);
    h.set_mode(0o644);
    h.set_uid(0);
    h.set_gid(0);
    h.set_mtime(0);
    h.set_size(body.len() as u64);
    h.set_cksum();
    builder.append_data(&mut h, "./control", body)?;
    builder.finish()?;
  }
  let mut gz = Vec::new();
  let mut enc = GzEncoder::new(&mut gz, Compression::default());
  std::io::Write::write_all(&mut enc, &tar_buf)?;
  enc.finish()?;
  Ok(gz)
}

/// Wrap a Linux app directory in an RPM `.rpm` package via the pure-Rust `rpm`
/// crate (no `rpmbuild`, so it cross-compiles). Same install layout as the
/// `.deb`. `Requires` is expressed as CEF shared-library sonames, which resolve
/// across RPM distros without hard-coding each one's package names.
fn create_linux_rpm(
  app_dir: &Path,
  rpm_path: &Path,
  desktop_flags: &DesktopFlags,
  target: Option<&str>,
) -> Result<(), AnyError> {
  let meta = linux_package_meta(app_dir, desktop_flags);
  let arch = rpm_arch_for_target(target)?;

  // Zstd payload (see Cargo.toml: gzip would pull flate2's zlib-rs shim, which
  // link-clashes with deno's zlib-ng), plus a pinned source date for
  // reproducible builds — mirrors the `mtime: 0` used in the AppImage/`.deb`
  // paths so identical inputs yield identical packages.
  let config = rpm::BuildConfig::default()
    .compression(rpm::CompressionType::Zstd)
    .source_date(0u32);

  let mut builder = rpm::PackageBuilder::new(
    &meta.package,
    &meta.version,
    "MIT",
    arch,
    &meta.summary,
  );
  builder.using_config(config);
  builder.description(&meta.summary);
  builder.vendor(&meta.maintainer);

  // Own /usr/lib/<pkg>/** (the staged app dir). Standard dirs (/usr, /usr/bin,
  // /usr/share, …) are deliberately not owned — they belong to the filesystem
  // package.
  builder
    .with_dir(app_dir, format!("/usr/lib/{}", meta.package), |o| o)
    .context("failed to add app directory to rpm")?;

  // /usr/bin/<pkg> → ../lib/<pkg>/<launcher>
  builder
    .with_symlink(rpm::FileOptions::symlink(
      format!("/usr/bin/{}", meta.package),
      format!("../lib/{}/{}", meta.package, meta.app_name),
    ))
    .context("failed to add launcher symlink to rpm")?;

  // /usr/share/applications/<pkg>.desktop — staged to a temp file because the
  // rpm builder reads file content from disk.
  let staging = tempfile::Builder::new()
    .prefix(".deno-desktop-rpm-")
    .tempdir()?;
  let desktop_path = staging.path().join("app.desktop");
  std::fs::write(&desktop_path, system_desktop_entry(&meta))?;
  builder
    .with_file(
      &desktop_path,
      rpm::FileOptions::new(format!(
        "/usr/share/applications/{}.desktop",
        meta.package
      )),
    )
    .context("failed to add .desktop file to rpm")?;

  // /usr/share/icons/.../<pkg>.png (if present)
  let icon_src = app_dir.join("AppIcon.png");
  if icon_src.exists() {
    builder
      .with_file(
        &icon_src,
        rpm::FileOptions::new(format!(
          "/usr/share/icons/hicolor/512x512/apps/{}.png",
          meta.package
        )),
      )
      .context("failed to add icon to rpm")?;
  }

  // RPM auto-`Provides` 64-bit ELF sonames with an `()(64bit)` class suffix
  // (e.g. `libgtk-3.so.0()(64bit)`), so a bare-soname `Requires` would not
  // match. Both supported arches (x86_64, aarch64) are 64-bit ELF, so always
  // append the suffix.
  for (soname, _) in CEF_RUNTIME_DEPS {
    builder.requires(rpm::Dependency::any(format!("{soname}()(64bit)")));
  }

  let package = builder.build().context("failed to build rpm package")?;

  if let Some(parent) = rpm_path.parent()
    && !parent.as_os_str().is_empty()
  {
    std::fs::create_dir_all(parent)?;
  }
  let mut out = std::fs::File::create(rpm_path).with_context(|| {
    format!("Failed to create .rpm at {}", rpm_path.display())
  })?;
  package.write(&mut out).with_context(|| {
    format!("Failed to write .rpm at {}", rpm_path.display())
  })?;
  Ok(())
}

// ===================== Windows .msi installer =========================== //

/// Fixed namespace for deriving deterministic MSI GUIDs (ProductCode,
/// UpgradeCode, package code, component GUIDs) from the app identity via
/// UUIDv5. Deterministic GUIDs keep `.msi` builds reproducible (an
/// identical app produces an identical installer) and give a stable
/// `UpgradeCode` across versions so newer installers can detect and replace
/// older ones — mirroring the `source_date(0)` / `mtime: 0` reproducibility of
/// the `.rpm` / `.deb` / AppImage paths.
const MSI_GUID_NAMESPACE: uuid::Uuid =
  uuid::Uuid::from_u128(0x6f1d3c8a_4b2e_4f5a_9c7d_8e0f1a2b3c4d);

/// Format a UUID as an MSI registry-format GUID: braced, uppercase, hyphenated
/// (e.g. `{6F1D3C8A-4B2E-4F5A-9C7D-8E0F1A2B3C4D}`). This is what the `GUID`
/// column category requires (38 chars, no lowercase).
fn msi_guid(uuid: uuid::Uuid) -> String {
  format!("{{{}}}", uuid.as_hyphenated().to_string().to_uppercase())
}

/// Derive a deterministic GUID for a given role (e.g. "product:1.0.0",
/// "upgrade", "package", "component:c0") of the named app.
fn msi_derive_guid(identifier: &str, role: &str) -> String {
  let name = format!("{identifier}\0{role}");
  msi_guid(uuid::Uuid::new_v5(&MSI_GUID_NAMESPACE, name.as_bytes()))
}

/// Map a target triple (or the host arch) to the MSI summary-info architecture
/// string used in the `Template` field. Both supported arches are 64-bit.
fn msi_arch_for_target(target: Option<&str>) -> Result<&'static str, AnyError> {
  let arch = target
    .and_then(|t| t.split('-').next())
    .unwrap_or(std::env::consts::ARCH);
  match arch {
    "x86_64" => Ok("x64"),
    "aarch64" => Ok("Arm64"),
    other => bail!(
      "No MSI architecture mapping for arch '{other}'; supported: x86_64, aarch64"
    ),
  }
}

/// Lowercase base36 encoding of a counter, used to mint unique 8.3 short names.
fn base36(mut n: u32) -> String {
  if n == 0 {
    return "0".to_string();
  }
  const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
  let mut out = Vec::new();
  while n > 0 {
    out.push(DIGITS[(n % 36) as usize]);
    n /= 36;
  }
  out.reverse();
  String::from_utf8(out).unwrap()
}

/// Mint a unique DOS 8.3 short name for the MSI `DefaultDir` / `File.FileName`
/// "short|long" syntax. The long name carries the real (possibly long) name;
/// the short name only has to be a unique, valid 8.3 token within its
/// directory. We derive it from a monotonically increasing counter (globally
/// unique ⇒ unique within any directory) so we never have to reconcile
/// collisions: `F<base36>` (files, ≤8 chars) plus the real extension truncated
/// to 3 uppercased alphanumerics, or `D<base36>` (directories, no extension).
fn msi_short_name(counter: u32, long: &str, is_dir: bool) -> String {
  let token = base36(counter).to_uppercase();
  if is_dir {
    format!("D{token}")
  } else {
    let ext: String = long
      .rsplit_once('.')
      .map(|(_, e)| e)
      .unwrap_or("")
      .chars()
      .filter(|c| c.is_ascii_alphanumeric())
      .take(3)
      .collect::<String>()
      .to_uppercase();
    if ext.is_empty() {
      format!("F{token}")
    } else {
      format!("F{token}.{ext}")
    }
  }
}

/// A staged file destined for both the embedded cabinet and the MSI `File`
/// table.
struct MsiFile {
  /// MSI `File` primary key (also the file's name inside the cabinet).
  key: String,
  /// Component the file belongs to (one per install directory).
  component: String,
  /// `short|long` `FileName` value.
  file_name: String,
  size: u64,
  abs_path: PathBuf,
}

/// Wrap a Windows app directory in a Windows Installer `.msi` package.
///
/// The MSI database is authored entirely in pure Rust via the `msi` crate, with
/// the file payload stored in an embedded MSZIP cabinet (`cab` crate), so it
/// cross-compiles from any host — only the *target* must be Windows. The app is
/// installed per-machine under `ProgramFiles64Folder\<AppName>\`, mirroring the
/// staged app-dir tree exactly; uninstall removes it. Layout:
///
/// ```text
/// %ProgramFiles%\<AppName>\
///   <AppName>.bat          (launcher)
///   laufey.exe             (CEF backend)
///   libcef.dll, ...        (CEF support files)
///   denort.dll             (compiled runtime + user code)
///   ...                    (nested dirs preserved)
/// ```
fn create_windows_msi(
  app_dir: &Path,
  msi_path: &Path,
  desktop_flags: &DesktopFlags,
  target: Option<&str>,
) -> Result<(), AnyError> {
  use msi::CodePage;
  use msi::Column;
  use msi::Insert;
  use msi::Package;
  use msi::PackageType;
  use msi::Value;

  let app_name = app_dir
    .file_name()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| "App".to_string());
  // Version default matches the macOS bundle's CFBundleVersion and the
  // Linux package default.
  let version = "1.0.0";
  let identifier = desktop_flags
    .identifier
    .clone()
    .unwrap_or_else(|| format!("com.deno.desktop.{}", app_name.to_lowercase()));
  let manufacturer = "Deno";
  let arch = msi_arch_for_target(target)?;

  // --- Walk the staged tree: register every directory, then every file. ----
  // Directories are keyed by their path relative to `app_dir` ("" = the
  // install root, INSTALLDIR). Sorted traversal keeps IDs deterministic.
  let mut rel_files: Vec<(PathBuf, u64)> = Vec::new();
  let mut rel_dirs: std::collections::BTreeSet<PathBuf> =
    std::collections::BTreeSet::new();
  let mut stack = vec![app_dir.to_path_buf()];
  while let Some(dir) = stack.pop() {
    let mut entries: Vec<_> =
      std::fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
      let path = entry.path();
      let md = std::fs::symlink_metadata(&path)?;
      if md.is_dir() {
        rel_dirs.insert(path.strip_prefix(app_dir)?.to_path_buf());
        stack.push(path);
      } else if md.is_file() {
        let rel = path.strip_prefix(app_dir)?.to_path_buf();
        rel_files.push((rel, md.len()));
      }
      // Symlinks are not expected in a Windows app dir (copy_dir_all
      // dereferences) — skip anything else.
    }
  }
  rel_files.sort();

  // Assign a Directory id to every directory. Root → INSTALLDIR; nested dirs →
  // d0, d1, … in sorted order.
  let mut dir_ids: std::collections::BTreeMap<PathBuf, String> =
    std::collections::BTreeMap::new();
  dir_ids.insert(PathBuf::new(), "INSTALLDIR".to_string());
  for (i, dir) in rel_dirs.iter().enumerate() {
    dir_ids.insert(dir.clone(), format!("d{i}"));
  }

  // The Program Files (64-bit) root that hosts the install dir. Both supported
  // arches are 64-bit, so we always target the 64-bit Program Files.
  let pf_folder = "ProgramFiles64Folder";

  // --- Directory table rows. ----------------------------------------------
  let mut short_counter: u32 = 0;
  let mut directory_rows: Vec<Vec<Value>> = vec![
    vec![
      Value::Str("TARGETDIR".to_string()),
      Value::Null,
      Value::Str("SourceDir".to_string()),
    ],
    vec![
      Value::Str(pf_folder.to_string()),
      Value::Str("TARGETDIR".to_string()),
      Value::Str(".".to_string()),
    ],
    vec![
      Value::Str("INSTALLDIR".to_string()),
      Value::Str(pf_folder.to_string()),
      Value::Str(format!(
        "{}|{}",
        msi_short_name(
          {
            short_counter += 1;
            short_counter
          },
          &app_name,
          true
        ),
        app_name
      )),
    ],
  ];
  for dir in &rel_dirs {
    let id = dir_ids[dir].clone();
    let parent = dir_ids[dir.parent().unwrap_or(Path::new(""))].clone();
    let name = dir
      .file_name()
      .map(|s| s.to_string_lossy().into_owned())
      .unwrap_or_else(|| id.clone());
    short_counter += 1;
    directory_rows.push(vec![
      Value::Str(id),
      Value::Str(parent),
      Value::Str(format!(
        "{}|{}",
        msi_short_name(short_counter, &name, true),
        name
      )),
    ]);
  }

  // --- Components: one per directory that directly contains files. ---------
  // A component's KeyPath is its first (sorted) file. Files inherit their
  // directory's component, so files always install next to their siblings.
  let mut comp_for_dir: std::collections::BTreeMap<PathBuf, String> =
    std::collections::BTreeMap::new();
  let mut files: Vec<MsiFile> = Vec::new();
  for (rel, size) in &rel_files {
    let dir = rel.parent().unwrap_or(Path::new("")).to_path_buf();
    let next_comp = format!("c{}", comp_for_dir.len());
    let component =
      comp_for_dir.entry(dir.clone()).or_insert(next_comp).clone();
    let key = format!("f{}", files.len());
    let long = rel
      .file_name()
      .map(|s| s.to_string_lossy().into_owned())
      .unwrap_or_else(|| key.clone());
    short_counter += 1;
    files.push(MsiFile {
      key,
      component,
      file_name: format!(
        "{}|{}",
        msi_short_name(short_counter, &long, false),
        long
      ),
      size: *size,
      abs_path: app_dir.join(rel),
    });
  }
  if files.is_empty() {
    bail!("Cannot build a .msi from an empty app directory");
  }

  // Locate the laufey launcher in the install root so we can author a Start Menu
  // shortcut to it (otherwise the installed app is not discoverable — there is no
  // icon anywhere, only files under Program Files). The launcher is the backend
  // binary renamed to `<app>.exe`; it auto-loads the co-located `<app>.dll`
  // runtime, so the shortcut targets it directly with no arguments.
  let launcher_exe = format!("{app_name}.exe").to_ascii_lowercase();
  let shortcut_target = rel_files
    .iter()
    .zip(files.iter())
    .find(|((rel, _), _)| {
      rel.parent() == Some(Path::new(""))
        && rel
          .file_name()
          .and_then(|n| n.to_str())
          .is_some_and(|n| n.to_ascii_lowercase() == launcher_exe)
    })
    .map(|(_, f)| (f.key.clone(), f.component.clone()));

  // The all-users Start Menu folder that hosts the app shortcut. Only added when
  // we found a launcher to point at.
  if shortcut_target.is_some() {
    directory_rows.push(vec![
      Value::Str("ProgramMenuFolder".to_string()),
      Value::Str("TARGETDIR".to_string()),
      Value::Str(".".to_string()),
    ]);
  }

  // msidbComponentAttributes64bit (256): mark components 64-bit so they
  // resolve ProgramFiles64Folder and the 64-bit registry view.
  const COMPONENT_64BIT: i32 = 256;
  let component_rows: Vec<Vec<Value>> = comp_for_dir
    .iter()
    .map(|(dir, comp)| {
      let dir_id = dir_ids[dir].clone();
      let keypath = files
        .iter()
        .find(|f| &f.component == comp)
        .map(|f| f.key.clone())
        .unwrap();
      vec![
        Value::Str(comp.clone()),
        Value::Str(msi_derive_guid(&identifier, &format!("component:{comp}"))),
        Value::Str(dir_id),
        Value::Int(COMPONENT_64BIT),
        Value::Null,
        Value::Str(keypath),
      ]
    })
    .collect();

  // --- File table + cabinet payload (shared 1-based sequence). -------------
  // msidbFileAttributesVital (512): a failed file install aborts the
  // transaction. The summary Word Count marks the source compressed, so no
  // per-file Compressed attribute is needed.
  const FILE_VITAL: i32 = 512;
  let file_rows: Vec<Vec<Value>> = files
    .iter()
    .enumerate()
    .map(|(i, f)| {
      vec![
        Value::Str(f.key.clone()),
        Value::Str(f.component.clone()),
        Value::Str(f.file_name.clone()),
        Value::Int(f.size as i32),
        Value::Null, // Version (not a tracked-version file)
        Value::Null, // Language
        Value::Int(FILE_VITAL),
        Value::Int(1 + i as i32), // Sequence
      ]
    })
    .collect();

  // Build the embedded cabinet: a single MSZIP folder holding every file,
  // named by its MSI `File` key, in sequence order.
  let cab_bytes = build_msi_cabinet(&files)?;

  // --- Author the MSI database. -------------------------------------------
  let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
  let mut package = Package::create(PackageType::Installer, &mut cursor)?;

  // The `msi` crate defaults both the database string pool and the summary
  // info to the UTF-8 codepage (65001). Windows Installer (msiexec) rejects a
  // UTF-8 database outright — "This installation package could not be opened"
  // — because an MSI codepage must be a valid ANSI codepage (or neutral). All
  // our strings are ASCII (file names, GUIDs, ids), so Windows-1252 encodes
  // them identically and is universally accepted. Set the database codepage
  // here and the summary-info codepage below.
  package.set_database_codepage(CodePage::Windows1252);

  {
    let summary = package.summary_info_mut();
    summary.set_codepage(CodePage::Windows1252);
    summary.set_title(format!("{app_name} Installer"));
    summary.set_subject(app_name.clone());
    summary.set_author(manufacturer.to_string());
    summary.set_comments(format!("{app_name} desktop application"));
    summary.set_arch(arch);
    summary.set_languages(&[msi::Language::from_code(1033)]);
    summary.set_creating_application("deno desktop");
    // Package code: a fresh GUID identifying this exact package build.
    summary.set_uuid(uuid::Uuid::new_v5(
      &MSI_GUID_NAMESPACE,
      format!("{identifier}\0package:{version}").as_bytes(),
    ));
    // Word Count bit 1 (2) = source files are compressed (in cabinets); bit 0
    // clear = long file names allowed.
    summary.set_word_count(2);
    // Page Count = minimum Windows Installer version (2.00).
    summary.set_page_count(200);
    // Fixed creation time (2020-01-01) for reproducible output.
    summary.set_creation_time(
      std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800),
    );
  }

  // Table schemas (subset of the Windows Installer schema needed to install
  // and uninstall a per-machine app).
  package.create_table(
    "Directory",
    vec![
      Column::build("Directory").primary_key().id_string(72),
      Column::build("Directory_Parent").nullable().id_string(72),
      Column::build("DefaultDir")
        .category(msi::Category::DefaultDir)
        .string(255),
    ],
  )?;
  package.create_table(
    "Component",
    vec![
      Column::build("Component").primary_key().id_string(72),
      Column::build("ComponentId")
        .nullable()
        .category(msi::Category::Guid)
        .string(38),
      Column::build("Directory_").id_string(72),
      Column::build("Attributes").int16(),
      Column::build("Condition")
        .nullable()
        .category(msi::Category::Condition)
        .string(255),
      Column::build("KeyPath").nullable().id_string(72),
    ],
  )?;
  package.create_table(
    "Feature",
    vec![
      Column::build("Feature").primary_key().id_string(38),
      Column::build("Feature_Parent").nullable().id_string(38),
      Column::build("Title").nullable().text_string(64),
      Column::build("Description").nullable().text_string(255),
      Column::build("Display").nullable().int16(),
      Column::build("Level").int16(),
      Column::build("Directory_").nullable().id_string(72),
      Column::build("Attributes").int16(),
    ],
  )?;
  package.create_table(
    "FeatureComponents",
    vec![
      Column::build("Feature_").primary_key().id_string(38),
      Column::build("Component_").primary_key().id_string(72),
    ],
  )?;
  package.create_table(
    "File",
    vec![
      Column::build("File").primary_key().id_string(72),
      Column::build("Component_").id_string(72),
      Column::build("FileName")
        .category(msi::Category::Filename)
        .string(255),
      Column::build("FileSize").int32(),
      Column::build("Version")
        .nullable()
        .category(msi::Category::Version)
        .string(72),
      Column::build("Language").nullable().string(20),
      Column::build("Attributes").nullable().int16(),
      Column::build("Sequence").int16(),
    ],
  )?;
  package.create_table(
    "Media",
    vec![
      Column::build("DiskId").primary_key().int16(),
      Column::build("LastSequence").int16(),
      Column::build("DiskPrompt").nullable().text_string(64),
      Column::build("Cabinet")
        .nullable()
        .category(msi::Category::Cabinet)
        .string(255),
      Column::build("VolumeLabel").nullable().text_string(32),
      Column::build("Source")
        .nullable()
        .category(msi::Category::Property)
        .string(72),
    ],
  )?;
  package.create_table(
    "Property",
    vec![
      Column::build("Property").primary_key().id_string(72),
      Column::build("Value").text_string(0),
    ],
  )?;
  if shortcut_target.is_some() {
    package.create_table(
      "Shortcut",
      vec![
        Column::build("Shortcut").primary_key().id_string(72),
        Column::build("Directory_").id_string(72),
        Column::build("Name")
          .category(msi::Category::Filename)
          .string(128),
        Column::build("Component_").id_string(72),
        Column::build("Target")
          .category(msi::Category::Shortcut)
          .string(72),
        Column::build("Arguments")
          .nullable()
          .category(msi::Category::Formatted)
          .string(255),
        Column::build("Description").nullable().text_string(255),
        Column::build("Hotkey").nullable().int16(),
        Column::build("Icon_").nullable().id_string(72),
        Column::build("IconIndex").nullable().int16(),
        Column::build("ShowCmd").nullable().int16(),
        Column::build("WkDir").nullable().id_string(72),
      ],
    )?;
  }
  for table in ["InstallExecuteSequence", "InstallUISequence"] {
    package.create_table(
      table,
      vec![
        Column::build("Action").primary_key().id_string(72),
        Column::build("Condition")
          .nullable()
          .category(msi::Category::Condition)
          .string(255),
        Column::build("Sequence").nullable().int16(),
      ],
    )?;
  }

  // --- Populate tables. ----------------------------------------------------
  package.insert_rows(Insert::into("Directory").rows(directory_rows))?;
  package.insert_rows(Insert::into("Component").rows(component_rows))?;
  package.insert_rows(Insert::into("Feature").row(vec![
    Value::Str("MainFeature".to_string()),
    Value::Null,
    Value::Str(app_name.clone()),
    Value::Null,
    Value::Int(1),
    Value::Int(1),
    Value::Str("INSTALLDIR".to_string()),
    Value::Int(0),
  ]))?;
  package.insert_rows(
    Insert::into("FeatureComponents").rows(
      comp_for_dir
        .values()
        .map(|c| {
          vec![Value::Str("MainFeature".to_string()), Value::Str(c.clone())]
        })
        .collect(),
    ),
  )?;
  package.insert_rows(Insert::into("File").rows(file_rows))?;
  package.insert_rows(Insert::into("Media").row(vec![
    Value::Int(1),
    Value::Int(files.len() as i32),
    Value::Null,
    Value::Str("#appcab".to_string()),
    Value::Null,
    Value::Null,
  ]))?;
  if let Some((launcher_key, launcher_comp)) = &shortcut_target {
    short_counter += 1;
    let short_name = msi_short_name(short_counter, &app_name, false);
    package.insert_rows(Insert::into("Shortcut").row(vec![
      Value::Str("AppShortcut".to_string()),
      Value::Str("ProgramMenuFolder".to_string()),
      Value::Str(format!("{short_name}|{app_name}")),
      Value::Str(launcher_comp.clone()),
      // Non-advertised shortcut: `[#key]` resolves to the installed exe's path.
      Value::Str(format!("[#{launcher_key}]")),
      Value::Null, // Arguments (co-located auto-load)
      Value::Null, // Description
      Value::Null, // Hotkey
      Value::Null, // Icon_
      Value::Null, // IconIndex
      Value::Null, // ShowCmd
      Value::Str("INSTALLDIR".to_string()), // WkDir
    ]))?;
  }

  let product_code =
    msi_derive_guid(&identifier, &format!("product:{version}"));
  let upgrade_code = msi_derive_guid(&identifier, "upgrade");
  package.insert_rows(Insert::into("Property").rows(vec![
    vec![
      Value::Str("ProductCode".to_string()),
      Value::Str(product_code),
    ],
    vec![
      Value::Str("ProductName".to_string()),
      Value::Str(app_name.clone()),
    ],
    vec![
      Value::Str("ProductVersion".to_string()),
      Value::Str(version.to_string()),
    ],
    vec![
      Value::Str("ProductLanguage".to_string()),
      Value::Str("1033".to_string()),
    ],
    vec![
      Value::Str("Manufacturer".to_string()),
      Value::Str(manufacturer.to_string()),
    ],
    vec![
      Value::Str("UpgradeCode".to_string()),
      Value::Str(upgrade_code),
    ],
    // Per-machine install (into Program Files).
    vec![
      Value::Str("ALLUSERS".to_string()),
      Value::Str("1".to_string()),
    ],
  ]))?;

  // Standard action sequences for a basic per-machine install + uninstall.
  let mut exec_seq: Vec<(&str, i32)> = vec![
    ("CostInitialize", 800),
    ("FileCost", 900),
    ("CostFinalize", 1000),
    ("InstallValidate", 1400),
    ("InstallInitialize", 1500),
    ("ProcessComponents", 1600),
    ("UnpublishFeatures", 1800),
    ("RemoveFiles", 3500),
    ("InstallFiles", 4000),
    ("RegisterProduct", 6100),
    ("PublishFeatures", 6300),
    ("PublishProduct", 6400),
    ("InstallFinalize", 6600),
  ];
  if shortcut_target.is_some() {
    // RemoveShortcuts on uninstall; CreateShortcuts after the files land.
    exec_seq.push(("RemoveShortcuts", 3800));
    exec_seq.push(("CreateShortcuts", 4500));
  }
  package.insert_rows(
    Insert::into("InstallExecuteSequence").rows(
      exec_seq
        .iter()
        .map(|(a, s)| {
          vec![Value::Str(a.to_string()), Value::Null, Value::Int(*s)]
        })
        .collect(),
    ),
  )?;
  let ui_seq: &[(&str, i32)] = &[
    ("CostInitialize", 800),
    ("FileCost", 900),
    ("CostFinalize", 1000),
    ("ExecuteAction", 1300),
  ];
  package.insert_rows(
    Insert::into("InstallUISequence").rows(
      ui_seq
        .iter()
        .map(|(a, s)| {
          vec![Value::Str(a.to_string()), Value::Null, Value::Int(*s)]
        })
        .collect(),
    ),
  )?;

  // Embed the cabinet as a stream named to match Media.Cabinet ("#appcab").
  {
    use std::io::Write as _;
    let mut stream = package.write_stream("appcab")?;
    stream.write_all(&cab_bytes)?;
  }

  package.flush()?;
  drop(package);

  if let Some(parent) = msi_path.parent()
    && !parent.as_os_str().is_empty()
  {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(msi_path, cursor.into_inner()).with_context(|| {
    format!("Failed to write .msi at {}", msi_path.display())
  })?;

  // The `msi` crate emits each table's rows in the order they were inserted (and
  // the auto-generated system tables `_Tables`/`_Columns`/`_Validation` in
  // table-name order), but Windows Installer requires every table's rows to be
  // sorted ascending by primary key — and for string-typed keys that ordering is
  // by the row's *string-pool id*, not the string's text. When the two orders
  // disagree (e.g. `_Validation` has a low string id yet sorts late
  // alphabetically) real `msiexec` rejects the database at open time with error
  // 2219 "Invalid Installer database format", even though the file is a valid
  // compound document and round-trips through the `msi` crate's own reader. Fix
  // this up in place by re-sorting every persistent table by its primary-key
  // columns in string-id order. See `msi_sort_tables_by_string_id`.
  msi_sort_tables_by_string_id(msi_path).with_context(|| {
    format!("Failed to finalize .msi at {}", msi_path.display())
  })?;
  Ok(())
}

/// Decode a Windows Installer stream name back to its logical table name.
///
/// MSI stores each table in a compound-file stream whose name is encoded with a
/// custom base-64 scheme: code points in `0x3800..0x4800` carry two base-64
/// digits and `0x4800..0x4840` carry one, over the alphabet
/// `0-9 A-Z a-z . _`. Table-data streams are additionally prefixed with the
/// sentinel `0x4840`, which falls outside both ranges; we return that prefix as
/// a leading NUL so callers can tell a real table stream apart from the special
/// `\u{5}SummaryInformation` stream.
fn msi_demangle_stream_name(name: &str) -> String {
  const B64: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz._";
  let mut out = String::new();
  for c in name.chars() {
    let v = c as u32;
    if (0x3800..0x4800).contains(&v) {
      let n = v - 0x3800;
      out.push(B64[(n & 0x3f) as usize] as char);
      out.push(B64[((n >> 6) & 0x3f) as usize] as char);
    } else if (0x4800..0x4840).contains(&v) {
      let n = v - 0x4800;
      out.push(B64[(n & 0x3f) as usize] as char);
    } else if v == 0x4840 {
      out.push('\0');
    } else {
      out.push(c);
    }
  }
  out
}

/// Re-sort the rows of a column-major MSI table stream by the given key columns.
///
/// `widths` is the byte width of every column (a stream is laid out column by
/// column: all of column 0's values, then all of column 1's, …). `keys` lists
/// the column indices to sort by, in priority order. Stored values are compared
/// as little-endian unsigned integers, which matches MSI's ordering for both
/// integer columns and string columns (whose stored value is the string-pool
/// id).
fn msi_resort_stream(data: &[u8], widths: &[usize], keys: &[usize]) -> Vec<u8> {
  let row_width: usize = widths.iter().sum();
  if row_width == 0 {
    return data.to_vec();
  }
  let rows = data.len() / row_width;
  if rows <= 1 {
    return data.to_vec();
  }
  // Byte offset where each column's run of values begins.
  let mut col_off = vec![0usize; widths.len()];
  let mut acc = 0;
  for (c, &w) in widths.iter().enumerate() {
    col_off[c] = acc;
    acc += rows * w;
  }
  let read = |row: usize, col: usize| -> u64 {
    let base = col_off[col] + row * widths[col];
    let mut v = 0u64;
    for k in 0..widths[col] {
      v |= (data[base + k] as u64) << (8 * k);
    }
    v
  };
  let mut order: Vec<usize> = (0..rows).collect();
  order.sort_by(|&a, &b| {
    for &k in keys {
      match read(a, k).cmp(&read(b, k)) {
        std::cmp::Ordering::Equal => continue,
        ord => return ord,
      }
    }
    a.cmp(&b)
  });
  let mut out = vec![0u8; data.len()];
  for (c, &w) in widths.iter().enumerate() {
    for (new_row, &old_row) in order.iter().enumerate() {
      let src = col_off[c] + old_row * w;
      let dst = col_off[c] + new_row * w;
      out[dst..dst + w].copy_from_slice(&data[src..src + w]);
    }
  }
  out
}

/// Sort every persistent table in a freshly written `.msi` by primary key in
/// string-pool-id order, as Windows Installer requires (see the call site for
/// why the `msi` crate's output needs this). Operates in place on the compound
/// file, rewriting only the small metadata/table streams — the embedded cabinet
/// stream is left untouched.
fn msi_sort_tables_by_string_id(msi_path: &Path) -> Result<(), AnyError> {
  use std::io::Read;
  use std::io::Write;

  let mut comp = cfb::open_rw(msi_path)?;
  let names: Vec<String> = comp
    .read_storage("/")?
    .map(|e| e.name().to_string())
    .collect();
  let read_stream = |comp: &mut cfb::CompoundFile<std::fs::File>,
                     raw: &str|
   -> Result<Vec<u8>, AnyError> {
    let mut s = comp.open_stream(format!("/{raw}"))?;
    let mut b = Vec::new();
    s.read_to_end(&mut b)?;
    Ok(b)
  };
  let write_stream = |comp: &mut cfb::CompoundFile<std::fs::File>,
                      raw: &str,
                      bytes: &[u8]|
   -> Result<(), AnyError> {
    let mut s = comp.open_stream(format!("/{raw}"))?;
    s.write_all(bytes)?;
    Ok(())
  };
  let find_raw = |suffix: &str| -> Option<String> {
    names
      .iter()
      .find(|n| msi_demangle_stream_name(n).ends_with(suffix))
      .cloned()
  };

  // The string-pool header's top bit selects 3-byte string references; tiny
  // databases use 2-byte refs, but honor the flag to stay correct at any size.
  let pool_raw = find_raw("_StringPool").ok_or_else(|| {
    deno_core::anyhow::anyhow!("malformed .msi: no _StringPool stream")
  })?;
  let pool = read_stream(&mut comp, &pool_raw)?;
  let str_w: usize = if pool.len() >= 4 && (pool[3] & 0x80) != 0 {
    3
  } else {
    2
  };

  // Map every string-pool id to its text so data-table streams (named by table
  // text) can be matched to their column schema (keyed by table string id).
  let data_raw = find_raw("_StringData").ok_or_else(|| {
    deno_core::anyhow::anyhow!("malformed .msi: no _StringData stream")
  })?;
  let str_data = read_stream(&mut comp, &data_raw)?;
  let mut strings = vec![String::new()]; // 1-based; index 0 is unused.
  {
    let mut off = 0usize;
    let mut i = 4;
    while i + 4 <= pool.len() {
      let len = u16::from_le_bytes([pool[i], pool[i + 1]]) as usize;
      let end = (off + len).min(str_data.len());
      strings.push(String::from_utf8_lossy(&str_data[off..end]).into_owned());
      off += len;
      i += 4;
    }
  }
  let id_of = |text: &str| -> Option<u64> {
    strings.iter().position(|s| s == text).map(|i| i as u64)
  };

  // System tables have a fixed schema not described in `_Columns`; their
  // string-typed columns use the same `str_w` reference width.
  let system_tables: &[(&str, Vec<usize>, Vec<usize>)] = &[
    ("_Tables", vec![str_w], vec![0]),
    ("_Columns", vec![str_w, 2, str_w, 2], vec![0, 1]),
    (
      "_Validation",
      vec![str_w, str_w, str_w, 4, 4, str_w, 2, str_w, str_w, str_w],
      vec![0, 1],
    ),
  ];
  for (name, widths, keys) in system_tables {
    if let Some(raw) = find_raw(name) {
      let bytes = read_stream(&mut comp, &raw)?;
      let sorted = msi_resort_stream(&bytes, widths, keys);
      write_stream(&mut comp, &raw, &sorted)?;
    }
  }

  // Parse the (now sorted) `_Columns` table to recover every persistent table's
  // column widths and which columns are primary keys.
  let columns_raw = find_raw("_Columns").ok_or_else(|| {
    deno_core::anyhow::anyhow!("malformed .msi: no _Columns stream")
  })?;
  let columns = read_stream(&mut comp, &columns_raw)?;
  let col_row_w = str_w + 2 + str_w + 2; // Table, Number, Name, Type
  let ncol = columns.len() / col_row_w;
  let read_col = |arr_off: usize, row: usize, w: usize| -> u64 {
    let base = arr_off + row * w;
    let mut v = 0u64;
    for k in 0..w {
      v |= (columns[base + k] as u64) << (8 * k);
    }
    v
  };
  let off_table = 0usize;
  let off_number = ncol * str_w;
  let off_type = off_number + ncol * 2 + ncol * str_w;
  // A column Type word: 0x8000 marks the stored value present (always set here);
  // 0x0800 marks a string type; 0x2000 marks a primary-key column; for integer
  // columns the low byte is the storage size (2 or 4 bytes).
  let width_of = |ty: u64| -> usize {
    let t = (ty ^ 0x8000) & 0xffff;
    if (t & 0x0800) != 0 {
      str_w
    } else if (t & 0xff) == 4 {
      4
    } else {
      2
    }
  };
  let is_key = |ty: u64| -> bool { ((ty ^ 0x8000) & 0x2000) != 0 };
  let mut table_columns: std::collections::BTreeMap<u64, Vec<(u64, u64)>> =
    std::collections::BTreeMap::new();
  for r in 0..ncol {
    let table_id = read_col(off_table, r, str_w);
    let number = read_col(off_number, r, 2) ^ 0x8000;
    let ty = read_col(off_type, r, 2);
    table_columns
      .entry(table_id)
      .or_default()
      .push((number, ty));
  }
  for cols in table_columns.values_mut() {
    cols.sort_by_key(|&(number, _)| number);
  }

  // Re-sort each persistent data-table stream by its primary-key columns.
  for raw in &names {
    let demangled = msi_demangle_stream_name(raw);
    // Table-data streams carry the 0x4840 sentinel (decoded as a leading NUL);
    // skip the summary-information stream and the already-handled system tables.
    let Some(table_name) = demangled.strip_prefix('\0') else {
      continue;
    };
    if table_name.starts_with('_') {
      continue;
    }
    let Some(table_id) = id_of(table_name) else {
      continue;
    };
    let Some(cols) = table_columns.get(&table_id) else {
      continue;
    };
    let widths: Vec<usize> = cols.iter().map(|&(_, ty)| width_of(ty)).collect();
    let keys: Vec<usize> = cols
      .iter()
      .enumerate()
      .filter(|&(_, &(_, ty))| is_key(ty))
      .map(|(i, _)| i)
      .collect();
    if keys.is_empty() {
      continue;
    }
    let bytes = read_stream(&mut comp, raw)?;
    let sorted = msi_resort_stream(&bytes, &widths, &keys);
    write_stream(&mut comp, raw, &sorted)?;
  }

  comp.flush()?;
  Ok(())
}

/// Build the embedded MSZIP cabinet carrying every install file, named by its
/// MSI `File` key, in sequence order.
fn build_msi_cabinet(files: &[MsiFile]) -> Result<Vec<u8>, AnyError> {
  use std::io::Write as _;

  let mut builder = cab::CabinetBuilder::new();
  {
    let folder = builder.add_folder(cab::CompressionType::MsZip);
    for f in files {
      folder.add_file(f.key.clone());
    }
  }

  let cursor = std::io::Cursor::new(Vec::<u8>::new());
  let mut writer = builder
    .build(cursor)
    .context("failed to start cabinet for .msi")?;
  // Files come back in the order they were added; read each one's bytes by its
  // cab name (the MSI File key) to stay robust to ordering.
  let by_key: std::collections::HashMap<&str, &MsiFile> =
    files.iter().map(|f| (f.key.as_str(), f)).collect();
  while let Some(mut file_writer) = writer
    .next_file()
    .context("failed to advance cabinet writer")?
  {
    let name = file_writer.file_name().to_string();
    let f = by_key.get(name.as_str()).ok_or_else(|| {
      deno_core::anyhow::anyhow!("cabinet file {name} missing")
    })?;
    let data = std::fs::read(&f.abs_path).with_context(|| {
      format!("failed to read {} for .msi cabinet", f.abs_path.display())
    })?;
    file_writer.write_all(&data)?;
  }
  let cursor = writer.finish().context("failed to finish cabinet")?;
  Ok(cursor.into_inner())
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

  // --- macOS Info.plist ---

  #[test]
  fn macos_info_plist_includes_principal_class_and_tcc_usage_strings() {
    let permissions = [
      "microphone".to_string(),
      "camera".to_string(),
      "audioCapture".to_string(),
      "bluetooth".to_string(),
    ];
    let plist = render_macos_info_plist(
      "Deno Demo",
      "com.deno.demo",
      "laufey_webview",
      true,
      &permissions,
      false,
    );
    assert!(plist.contains("<string>LaufeyApplication</string>"));
    // The backend binary must be the CFBundleExecutable (not a shell-script
    // launcher named after the app) so LaunchServices launches the GUI
    // process directly — otherwise the tray icon never attaches. #35619.
    assert!(plist.contains(
      "<key>CFBundleExecutable</key>\n  <string>laufey_webview</string>"
    ));
    assert!(
      plist.contains("<key>NSMicrophoneUsageDescription</key>")
        && plist.contains("Deno Demo requires access to the microphone")
    );
    assert!(
      plist.contains("<key>NSCameraUsageDescription</key>")
        && plist.contains("Deno Demo requires access to the camera")
    );
    assert!(
      plist.contains("<key>NSAudioCaptureUsageDescription</key>")
        && plist.contains("Deno Demo requires access to audio capture")
    );
    assert!(
      plist.contains("<key>NSBluetoothAlwaysUsageDescription</key>")
        && plist.contains("Deno Demo requires access to Bluetooth")
    );
    assert!(plist.contains("<string>AppIcon</string>"));
  }

  #[test]
  fn macos_info_plist_omits_usage_keys_without_permissions() {
    // A plain browser/menu-bar utility declares no permissions, so none of the
    // camera/mic/audio/bluetooth usage-description keys should be emitted.
    let plist = render_macos_info_plist(
      "Async Link",
      "com.async.link",
      "laufey_webview",
      false,
      &[],
      false,
    );
    for key in &[
      "NSMicrophoneUsageDescription",
      "NSCameraUsageDescription",
      "NSAudioCaptureUsageDescription",
      "NSBluetoothAlwaysUsageDescription",
      "NSBluetoothPeripheralUsageDescription",
    ] {
      assert!(
        !plist.contains(key),
        "{key} should not be emitted when no permissions are declared"
      );
    }
    // The loopback transport keys stay regardless.
    assert!(plist.contains("<key>NSAllowsLocalNetworking</key>"));
    // No permissions requested, so LSUIElement stays off.
    assert!(!plist.contains("<key>LSUIElement</key>"));
  }

  #[test]
  fn macos_info_plist_emits_only_declared_permissions() {
    // Declaring only "camera" emits exactly that usage key, nothing else.
    let plist = render_macos_info_plist(
      "Cam App",
      "com.cam.app",
      "laufey_webview",
      false,
      &["camera".to_string()],
      false,
    );
    assert!(plist.contains("<key>NSCameraUsageDescription</key>"));
    assert!(!plist.contains("<key>NSMicrophoneUsageDescription</key>"));
    assert!(!plist.contains("<key>NSBluetoothAlwaysUsageDescription</key>"));
  }

  #[test]
  fn macos_info_plist_agent_emits_lsuielement() {
    let plist = render_macos_info_plist(
      "Menu Bar App",
      "com.menu.app",
      "laufey_webview",
      false,
      &[],
      true,
    );
    assert!(plist.contains("<key>LSUIElement</key>\n  <true/>"));
  }

  #[test]
  fn macos_runtime_dylib_name_is_backend_specific() {
    // webview resolves a hardcoded libruntime.dylib via [NSBundle mainBundle];
    // cef resolves <executable-basename>.dylib next to the backend binary.
    assert_eq!(
      macos_runtime_dylib_name("webview", "laufey_webview"),
      "libruntime.dylib"
    );
    assert_eq!(
      macos_runtime_dylib_name("cef", "laufey_cef"),
      "laufey_cef.dylib"
    );
  }

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
    let contents = "abc123    file.tar.gz
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

  // --- parse_dev_server_url ---

  #[test]
  fn dev_server_url_matches() {
    assert_eq!(
      parse_dev_server_url("  ➜  Local:   http://localhost:5173/").as_deref(),
      Some("http://localhost:5173/")
    );
    assert_eq!(
      parse_dev_server_url("  Local:   https://localhost:5173/").as_deref(),
      Some("https://localhost:5173/")
    );
  }

  #[test]
  fn dev_server_url_matches_with_subpath() {
    assert_eq!(
      parse_dev_server_url("  Local:   http://localhost:5173/app").as_deref(),
      Some("http://localhost:5173/app")
    );
    assert_eq!(
      parse_dev_server_url("  Local:   http://192.168.1.1:5173").as_deref(),
      Some("http://192.168.1.1:5173")
    );
    assert_eq!(
      parse_dev_server_url("  Local:   https://localhost:5173/app").as_deref(),
      Some("https://localhost:5173/app")
    );
  }

  #[test]
  fn dev_server_url_no_match() {
    assert_eq!(parse_dev_server_url("Watching for file changes..."), None);
    assert_eq!(parse_dev_server_url(""), None);
    assert_eq!(
      parse_dev_server_url("  Network:  http://192.168.1.1:5173/"),
      None
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
    // Some backends (winit) don't ship a Frameworks/ subdir. The
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
    // `raw` is the public backend name; the dev binary is `laufey_winit`,
    // carrying the host executable suffix (`.exe` on Windows) — without it
    // LAUFEY_DEV_DIR could never resolve a backend on Windows.
    let p = tmp.path().join(format!(
      "target/release/laufey_winit{}",
      std::env::consts::EXE_SUFFIX
    ));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"binary").unwrap();
    let found = locate_dev_backend_binary(tmp.path(), "raw");
    assert_eq!(found.as_deref(), Some(p.as_path()));
  }

  #[test]
  fn locate_dev_app_bundle_skips_winit() {
    // winit don't ship as .app bundles. locate_dev_app_bundle
    // must short-circuit to None for those, never touching the
    // filesystem (so a misleading directory with the right name can't
    // accidentally match).
    let tmp = tempfile::tempdir().unwrap();
    assert!(locate_dev_app_bundle(tmp.path(), "raw").is_none());
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

  // --- Linux packaging ---

  #[test]
  fn debian_package_name_sanitizes() {
    assert_eq!(debian_package_name("MyApp"), "myapp");
    assert_eq!(debian_package_name("My App!"), "my-app");
    // Leading non-alphanumerics are stripped (Debian forbids leading -/+/.).
    assert_eq!(debian_package_name("--foo"), "foo");
    assert_eq!(debian_package_name("a"), "app", "too short falls back");
    assert_eq!(debian_package_name("___"), "app", "nothing usable");
    // Allowed punctuation is preserved.
    assert_eq!(debian_package_name("a.b+c-d"), "a.b+c-d");
  }

  #[test]
  fn package_arch_mappings() {
    assert_eq!(
      debian_arch_for_target(Some("x86_64-unknown-linux-gnu")).unwrap(),
      "amd64"
    );
    assert_eq!(
      debian_arch_for_target(Some("aarch64-unknown-linux-gnu")).unwrap(),
      "arm64"
    );
    assert!(debian_arch_for_target(Some("riscv64-unknown-linux-gnu")).is_err());
    assert_eq!(
      rpm_arch_for_target(Some("x86_64-unknown-linux-gnu")).unwrap(),
      "x86_64"
    );
    assert_eq!(
      rpm_arch_for_target(Some("aarch64-unknown-linux-gnu")).unwrap(),
      "aarch64"
    );
    assert!(rpm_arch_for_target(Some("mips-unknown-linux-gnu")).is_err());
  }

  /// Build a minimal staged Linux app dir like `package_linux_app_dir` does:
  /// a launcher script named after the app, a fake runtime lib, and an icon.
  fn fake_linux_app_dir(parent: &Path, app_name: &str) -> PathBuf {
    let app_dir = parent.join(app_name);
    std::fs::create_dir_all(&app_dir).unwrap();
    // The backend binary is renamed to `<app>` and auto-loads the co-located
    // `<app>.so`; there is no launcher shell script.
    std::fs::write(app_dir.join(app_name), b"\x7fELFfake-bin").unwrap();
    std::fs::write(app_dir.join(format!("{app_name}.so")), b"\x7fELFfake")
      .unwrap();
    std::fs::write(app_dir.join("AppIcon.png"), STUB_ICON_PNG).unwrap();
    app_dir
  }

  fn empty_desktop_flags() -> DesktopFlags {
    DesktopFlags {
      source_file: String::new(),
      output: None,
      args: vec![],
      target: None,
      icon: None,
      include: vec![],
      exclude: vec![],
      hmr: false,
      backend: None,
      all_targets: false,
      identifier: None,
      deep_links: Vec::new(),
      allow_web_schemes: false,
      macos_permissions: Vec::new(),
      agent: false,
      codesign_identity: None,
      inspect_renderer: None,
      compress: None,
      exclude_unused_npm: false,
    }
  }

  /// Read a gzip-compressed buffer to bytes.
  fn gunzip(data: &[u8]) -> Vec<u8> {
    let mut dec = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).unwrap();
    out
  }

  #[test]
  fn appimage_uses_runtime_supported_zstd_squashfs() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = fake_linux_app_dir(tmp.path(), "MyApp");
    let appimage_path = tmp.path().join("MyApp.AppImage");
    let target = Some("x86_64-unknown-linux-gnu");
    create_linux_appimage(&app_dir, &appimage_path, target).unwrap();

    let runtime_offset =
      appimage_runtime_for_target(target).unwrap().len() as u64;
    let appimage =
      std::io::BufReader::new(std::fs::File::open(&appimage_path).unwrap());
    let filesystem = backhand::FilesystemReader::from_reader_with_offset(
      appimage,
      runtime_offset,
    )
    .unwrap();
    assert_eq!(
      filesystem.compressor,
      backhand::compression::Compressor::Zstd
    );
  }

  #[test]
  fn deb_has_ar_members_and_control_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = fake_linux_app_dir(tmp.path(), "MyApp");
    let deb = tmp.path().join("MyApp.deb");
    let flags = empty_desktop_flags();
    create_linux_deb(&app_dir, &deb, &flags, Some("x86_64-unknown-linux-gnu"))
      .unwrap();

    let bytes = std::fs::read(&deb).unwrap();
    assert!(bytes.starts_with(b"!<arch>\n"), "missing ar global header");

    // Walk the ar members in order and collect (name, data).
    let mut members: Vec<(String, Vec<u8>)> = Vec::new();
    let mut pos = 8; // after "!<arch>\n"
    while pos + 60 <= bytes.len() {
      let header = &bytes[pos..pos + 60];
      let name = String::from_utf8_lossy(&header[..16]).trim().to_string();
      let size: usize = String::from_utf8_lossy(&header[48..58])
        .trim()
        .parse()
        .unwrap();
      assert_eq!(&header[58..60], b"`\n", "bad ar member magic");
      let data_start = pos + 60;
      members.push((name, bytes[data_start..data_start + size].to_vec()));
      // members are padded to an even length
      pos = data_start + size + (size % 2);
    }
    let names: Vec<&str> = members.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
      names,
      vec!["debian-binary", "control.tar.gz", "data.tar.gz"],
      "members must appear in dpkg's expected order"
    );
    assert_eq!(members[0].1, b"2.0\n");

    // control.tar.gz → ./control with the required fields.
    let control_tar = gunzip(&members[1].1);
    let mut archive = tar::Archive::new(&control_tar[..]);
    let mut control = String::new();
    for entry in archive.entries().unwrap() {
      let mut entry = entry.unwrap();
      if entry.path().unwrap().ends_with("control") {
        entry.read_to_string(&mut control).unwrap();
      }
    }
    assert!(control.contains("Package: myapp\n"), "control:\n{control}");
    assert!(control.contains("Architecture: amd64\n"));
    assert!(control.contains("Version: 1.0.0\n"));
    assert!(control.contains("Depends: libgtk-3-0,"));
    assert!(control.contains("Installed-Size: "));

    // data.tar.gz install layout. The `tar` crate strips the conventional
    // leading `./` (it skips `CurDir` path components), leaving root-relative
    // `usr/...` paths — which dpkg unpacks to `/usr/...` all the same.
    let data_tar = gunzip(&members[2].1);
    let mut archive = tar::Archive::new(&data_tar[..]);
    let mut paths: Vec<String> = Vec::new();
    let mut bin_link_target: Option<String> = None;
    for entry in archive.entries().unwrap() {
      let entry = entry.unwrap();
      let p = entry.path().unwrap().to_string_lossy().into_owned();
      if p == "usr/bin/myapp" {
        bin_link_target = entry
          .link_name()
          .unwrap()
          .map(|l| l.to_string_lossy().into_owned());
      }
      paths.push(p);
    }
    assert!(paths.iter().any(|p| p == "usr/lib/myapp/MyApp"));
    assert!(paths.iter().any(|p| p == "usr/lib/myapp/MyApp.so"));
    assert!(
      paths
        .iter()
        .any(|p| p == "usr/share/applications/myapp.desktop")
    );
    assert!(
      paths
        .iter()
        .any(|p| p == "usr/share/icons/hicolor/512x512/apps/myapp.png")
    );
    assert_eq!(
      bin_link_target.as_deref(),
      Some("../lib/myapp/MyApp"),
      "/usr/bin symlink must point at the staged launcher"
    );
  }

  #[test]
  fn rpm_parses_with_expected_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = fake_linux_app_dir(tmp.path(), "MyApp");
    let rpm_path = tmp.path().join("MyApp.rpm");
    let flags = empty_desktop_flags();
    create_linux_rpm(
      &app_dir,
      &rpm_path,
      &flags,
      Some("aarch64-unknown-linux-gnu"),
    )
    .unwrap();

    let bytes = std::fs::read(&rpm_path).unwrap();
    let pkg = rpm::Package::parse(&mut &bytes[..]).unwrap();
    assert_eq!(pkg.metadata.get_name().unwrap(), "myapp");
    assert_eq!(pkg.metadata.get_version().unwrap(), "1.0.0");
    assert_eq!(pkg.metadata.get_arch().unwrap(), "aarch64");

    let requires: Vec<String> = pkg
      .metadata
      .get_requires()
      .unwrap()
      .into_iter()
      .map(|d| d.name)
      .collect();
    assert!(
      requires.iter().any(|r| r == "libgtk-3.so.0()(64bit)"),
      "rpm Requires must carry CEF sonames with the 64-bit ELF class suffix, got: {requires:?}"
    );

    let files: Vec<String> = pkg
      .metadata
      .get_file_paths()
      .unwrap()
      .into_iter()
      .map(|p| p.to_string_lossy().into_owned())
      .collect();
    assert!(files.iter().any(|f| f == "/usr/lib/myapp/MyApp"));
    assert!(files.iter().any(|f| f == "/usr/bin/myapp"));
    assert!(
      files
        .iter()
        .any(|f| f == "/usr/share/applications/myapp.desktop")
    );
  }

  // --- Windows .msi packaging ---

  #[test]
  fn msi_arch_mappings() {
    assert_eq!(
      msi_arch_for_target(Some("x86_64-pc-windows-msvc")).unwrap(),
      "x64"
    );
    assert_eq!(
      msi_arch_for_target(Some("aarch64-pc-windows-msvc")).unwrap(),
      "Arm64"
    );
    assert!(msi_arch_for_target(Some("i686-pc-windows-msvc")).is_err());
  }

  #[test]
  fn msi_guids_are_deterministic_and_valid() {
    let a = msi_derive_guid("com.acme.myapp", "product:1.0.0");
    let b = msi_derive_guid("com.acme.myapp", "product:1.0.0");
    let c = msi_derive_guid("com.acme.myapp", "upgrade");
    assert_eq!(a, b, "same inputs must yield the same GUID");
    assert_ne!(a, c, "different roles must yield different GUIDs");
    // Registry-format GUID: 38 chars, braced, uppercase.
    assert_eq!(a.len(), 38);
    assert!(a.starts_with('{') && a.ends_with('}'));
    assert!(
      msi::Category::Guid.validate(&a),
      "must satisfy GUID category"
    );
  }

  #[test]
  fn msi_short_names_are_valid_8_3() {
    let d = msi_short_name(1, "Some Long Folder Name", true);
    assert_eq!(d, "D1");
    let f = msi_short_name(42, "libcef.dll", false);
    assert_eq!(f, "F16.DLL");
    // No extension → no dot.
    assert_eq!(msi_short_name(2, "LICENSE", false), "F2");
  }

  /// Build a minimal staged Windows app dir with a nested subdirectory, like
  /// `package_windows_app_dir` would produce.
  fn fake_windows_app_dir(parent: &Path, app_name: &str) -> PathBuf {
    let app_dir = parent.join(app_name);
    std::fs::create_dir_all(&app_dir).unwrap();
    // The backend binary is renamed to `<app>.exe` and auto-loads the
    // co-located `<app>.dll`; there is no `.bat` wrapper.
    std::fs::write(app_dir.join(format!("{app_name}.exe")), b"MZfake-exe")
      .unwrap();
    std::fs::write(app_dir.join(format!("{app_name}.dll")), b"MZfake-dll")
      .unwrap();
    let locales = app_dir.join("locales");
    std::fs::create_dir_all(&locales).unwrap();
    std::fs::write(locales.join("en-US.pak"), b"pakdata").unwrap();
    app_dir
  }

  #[test]
  fn msi_parses_with_expected_tables_and_payload() {
    use std::io::Read as _;

    let tmp = tempfile::tempdir().unwrap();
    let app_dir = fake_windows_app_dir(tmp.path(), "MyApp");
    let msi_path = tmp.path().join("MyApp.msi");
    let flags = empty_desktop_flags();
    create_windows_msi(
      &app_dir,
      &msi_path,
      &flags,
      Some("x86_64-pc-windows-msvc"),
    )
    .unwrap();

    let mut package = msi::open(&msi_path).unwrap();
    assert_eq!(package.summary_info().arch(), Some("x64"));
    // Windows Installer rejects a UTF-8 (65001) database codepage with "could
    // not be opened"; both the database and summary-info codepage must be a
    // valid ANSI codepage. We author Windows-1252.
    assert_eq!(package.database_codepage(), msi::CodePage::Windows1252);
    assert_eq!(
      package.summary_info().codepage(),
      msi::CodePage::Windows1252
    );

    // Property table: product identity.
    let mut props = std::collections::HashMap::new();
    for row in package.select_rows(msi::Select::table("Property")).unwrap() {
      props.insert(
        row["Property"].as_str().unwrap().to_string(),
        row["Value"].as_str().unwrap().to_string(),
      );
    }
    assert_eq!(props.get("ProductName").map(String::as_str), Some("MyApp"));
    assert_eq!(
      props.get("ProductVersion").map(String::as_str),
      Some("1.0.0")
    );
    assert_eq!(props.get("ALLUSERS").map(String::as_str), Some("1"));
    assert!(
      msi::Category::Guid.validate(props.get("ProductCode").unwrap()),
      "ProductCode must be a valid GUID"
    );
    assert!(msi::Category::Guid.validate(props.get("UpgradeCode").unwrap()));

    // Directory table: the install root and the nested locales dir.
    let dirs: Vec<String> = package
      .select_rows(msi::Select::table("Directory"))
      .unwrap()
      .map(|r| r["Directory"].as_str().unwrap().to_string())
      .collect();
    assert!(dirs.iter().any(|d| d == "INSTALLDIR"));
    assert!(dirs.iter().any(|d| d == "ProgramFiles64Folder"));

    // File table: every staged file, with a 1..N sequence.
    let mut files: Vec<(String, String, i32)> = package
      .select_rows(msi::Select::table("File"))
      .unwrap()
      .map(|r| {
        (
          r["File"].as_str().unwrap().to_string(),
          r["FileName"].as_str().unwrap().to_string(),
          r["Sequence"].as_int().unwrap(),
        )
      })
      .collect();
    files.sort_by_key(|f| f.2);
    assert_eq!(files.len(), 3, "3 files staged: {files:?}");
    let long_names: Vec<&str> = files
      .iter()
      .map(|(_, name, _)| name.rsplit('|').next().unwrap())
      .collect();
    assert!(long_names.contains(&"MyApp.exe"));
    assert!(long_names.contains(&"MyApp.dll"));
    assert!(long_names.contains(&"en-US.pak"));
    let seqs: Vec<i32> = files.iter().map(|f| f.2).collect();
    assert_eq!(seqs, vec![1, 2, 3], "sequence must be contiguous 1..N");

    // Two components: one for the root dir, one for locales/.
    let comps: Vec<String> = package
      .select_rows(msi::Select::table("Component"))
      .unwrap()
      .map(|r| r["Component"].as_str().unwrap().to_string())
      .collect();
    assert_eq!(comps.len(), 2, "root + locales components: {comps:?}");

    // The embedded cabinet referenced by Media.Cabinet.
    let cabinet = package
      .select_rows(msi::Select::table("Media"))
      .unwrap()
      .map(|r| r["Cabinet"].as_str().unwrap().to_string())
      .next()
      .unwrap();
    assert_eq!(cabinet, "#appcab");

    // Read the cabinet back and confirm a file's bytes round-trip. The cab
    // member name is the MSI File key.
    let key_for_dll = files
      .iter()
      .find(|(_, name, _)| name.ends_with("MyApp.dll"))
      .map(|(key, _, _)| key.clone())
      .unwrap();
    let mut cab_bytes = Vec::new();
    package
      .read_stream("appcab")
      .unwrap()
      .read_to_end(&mut cab_bytes)
      .unwrap();
    let mut cabinet =
      cab::Cabinet::new(std::io::Cursor::new(cab_bytes)).unwrap();
    let mut content = Vec::new();
    cabinet
      .read_file(&key_for_dll)
      .unwrap()
      .read_to_end(&mut content)
      .unwrap();
    assert_eq!(content, b"MZfake-dll");
  }

  #[test]
  fn msi_tables_sorted_by_string_id() {
    use std::io::Read as _;

    let tmp = tempfile::tempdir().unwrap();
    let app_dir = fake_windows_app_dir(tmp.path(), "SortApp");
    let msi_path = tmp.path().join("SortApp.msi");
    create_windows_msi(
      &app_dir,
      &msi_path,
      &empty_desktop_flags(),
      Some("x86_64-pc-windows-msvc"),
    )
    .unwrap();

    // Windows Installer requires every table's rows to be ordered ascending by
    // primary key, and for string-typed keys that ordering is by the row's
    // string-pool id (not the string's text). The `msi` crate does not emit the
    // system tables in that order, so `create_windows_msi` re-sorts them; real
    // `msiexec` rejects the database with error 2219 otherwise. Verify the
    // invariant on the `_Tables` stream, whose single column is the table name's
    // string id, and on the `Directory` data table's primary-key column.
    let mut comp = cfb::open(&msi_path).unwrap();
    let names: Vec<String> = comp
      .read_storage("/")
      .unwrap()
      .map(|e| e.name().to_string())
      .collect();
    let read_ids =
      |comp: &mut cfb::CompoundFile<std::fs::File>, suffix: &str| -> Vec<u16> {
        let raw = names
          .iter()
          .find(|n| msi_demangle_stream_name(n).ends_with(suffix))
          .cloned()
          .unwrap();
        let mut s = comp.open_stream(format!("/{raw}")).unwrap();
        let mut b = Vec::new();
        s.read_to_end(&mut b).unwrap();
        b.chunks_exact(2)
          .map(|c| u16::from_le_bytes([c[0], c[1]]))
          .collect()
      };

    let table_ids = read_ids(&mut comp, "_Tables");
    assert!(table_ids.len() > 1, "expected multiple tables");
    assert!(
      table_ids.windows(2).all(|w| w[0] < w[1]),
      "_Tables must be strictly ascending by string id: {table_ids:?}"
    );

    // The Directory table's first column is its primary key (the directory id).
    // Its run of values (the table is stored column-major, key column first)
    // must be ascending by string id.
    let dir_keys = read_ids(&mut comp, "\0Directory");
    let dir_rows = dir_keys.len() / 3; // Directory, Directory_Parent, DefaultDir
    let pk: Vec<u16> = dir_keys.into_iter().take(dir_rows).collect();
    assert!(
      pk.windows(2).all(|w| w[0] < w[1]),
      "Directory primary key must be ascending by string id: {pk:?}"
    );
  }

  #[test]
  fn msi_authors_start_menu_shortcut() {
    let tmp = tempfile::tempdir().unwrap();
    // fake_windows_app_dir stages `MyApp.exe`, the renamed backend binary the
    // shortcut points at.
    let app_dir = fake_windows_app_dir(tmp.path(), "MyApp");
    let msi_path = tmp.path().join("MyApp.msi");
    create_windows_msi(
      &app_dir,
      &msi_path,
      &empty_desktop_flags(),
      Some("x86_64-pc-windows-msvc"),
    )
    .unwrap();
    let mut package = msi::open(&msi_path).unwrap();

    // The all-users Start Menu folder must exist for the shortcut to land in it.
    let dirs: Vec<String> = package
      .select_rows(msi::Select::table("Directory"))
      .unwrap()
      .map(|r| r["Directory"].as_str().unwrap().to_string())
      .collect();
    assert!(dirs.iter().any(|d| d == "ProgramMenuFolder"));

    // Exactly one shortcut, targeting `<app>.exe` (`[#fkey]`) with no arguments
    // (it auto-loads the co-located `<app>.dll`) and running from INSTALLDIR —
    // no console window.
    let shortcuts: Vec<_> = package
      .select_rows(msi::Select::table("Shortcut"))
      .unwrap()
      .collect();
    assert_eq!(shortcuts.len(), 1);
    let s = &shortcuts[0];
    assert_eq!(s["Directory_"].as_str(), Some("ProgramMenuFolder"));
    assert_eq!(s["Arguments"].as_str(), None);
    assert_eq!(s["WkDir"].as_str(), Some("INSTALLDIR"));
    assert!(s["Target"].as_str().unwrap().starts_with("[#"));

    // CreateShortcuts/RemoveShortcuts must be sequenced or the table is ignored.
    let actions: Vec<String> = package
      .select_rows(msi::Select::table("InstallExecuteSequence"))
      .unwrap()
      .map(|r| r["Action"].as_str().unwrap().to_string())
      .collect();
    assert!(actions.iter().any(|a| a == "CreateShortcuts"));
    assert!(actions.iter().any(|a| a == "RemoveShortcuts"));
  }

  // --- deep links ---

  #[test]
  fn validate_url_scheme_accepts_canonical() {
    for ok in &["acme", "my-app", "x", "com.acme.app", "a1+2.3-4", "App"] {
      assert!(
        validate_url_scheme(ok, false).is_ok(),
        "{ok:?} should be accepted"
      );
    }
  }

  #[test]
  fn validate_url_scheme_rejects_bad_shapes() {
    let cases = &[
      ("", "empty"),
      ("1acme", "leading digit"),
      ("-acme", "leading hyphen"),
      (".acme", "leading dot"),
      ("acme app", "space"),
      ("acme/app", "slash"),
      ("acme://", "colon and slashes"),
      ("acme_app", "underscore"),
      ("acmé", "non-ascii"),
    ];
    for (bad, why) in cases {
      assert!(
        validate_url_scheme(bad, false).is_err(),
        "{bad:?} should be rejected ({why})"
      );
    }
  }

  #[test]
  fn validate_url_scheme_rejects_reserved() {
    // Registering these as app handlers would hijack normal browsing.
    for reserved in &["http", "https", "file", "ftp", "ws", "wss"] {
      assert!(
        validate_url_scheme(reserved, false).is_err(),
        "{reserved:?} must be rejected as reserved"
      );
    }
  }

  #[test]
  fn validate_url_scheme_allow_web_schemes_permits_http() {
    // With the opt-in, http/https become registerable (default-browser case).
    for ok in &["http", "https"] {
      assert!(
        validate_url_scheme(ok, true).is_ok(),
        "{ok:?} should be accepted with allow_web_schemes"
      );
    }
    // The remaining reserved schemes stay reserved even with the opt-in.
    for still_reserved in &["file", "ftp", "ws", "wss"] {
      assert!(
        validate_url_scheme(still_reserved, true).is_err(),
        "{still_reserved:?} must stay reserved even with allow_web_schemes"
      );
    }
  }

  /// Build a minimal macOS `.app` skeleton with an `Info.plist` carrying the
  /// given bundle id, returning the bundle root.
  fn fake_macos_bundle(
    parent: &std::path::Path,
    bundle_id: &str,
  ) -> std::path::PathBuf {
    let bundle = parent.join("MyApp.app");
    let contents = bundle.join("Contents");
    std::fs::create_dir_all(&contents).unwrap();
    write_helper_plist(&contents.join("Info.plist"), bundle_id);
    bundle
  }

  #[test]
  fn deep_links_macos_writes_cfbundleurltypes() {
    let tmp = tempfile::tempdir().unwrap();
    let bundle = fake_macos_bundle(tmp.path(), "com.acme.myapp");
    register_deep_links_macos(
      &bundle,
      &["acme".to_string(), "acme-beta".to_string()],
    )
    .unwrap();

    let dict: plist::Dictionary =
      plist::from_file(bundle.join("Contents/Info.plist")).unwrap();
    let url_types = dict
      .get("CFBundleURLTypes")
      .and_then(|v| v.as_array())
      .expect("CFBundleURLTypes array");
    assert_eq!(url_types.len(), 1);
    let url_type = url_types[0].as_dictionary().unwrap();
    // The URL name is keyed off the bundle id so multiple apps don't collide.
    assert_eq!(
      url_type.get("CFBundleURLName").and_then(|v| v.as_string()),
      Some("com.acme.myapp")
    );
    assert_eq!(
      url_type.get("CFBundleTypeRole").and_then(|v| v.as_string()),
      Some("Viewer")
    );
    let schemes: Vec<&str> = url_type
      .get("CFBundleURLSchemes")
      .and_then(|v| v.as_array())
      .unwrap()
      .iter()
      .map(|v| v.as_string().unwrap())
      .collect();
    assert_eq!(schemes, ["acme", "acme-beta"]);
  }

  /// Write a minimal Linux `.desktop` entry into `dir` and return its path.
  fn fake_desktop_file(
    dir: &std::path::Path,
    body: &str,
  ) -> std::path::PathBuf {
    let path = dir.join("myapp.desktop");
    std::fs::write(&path, body).unwrap();
    path
  }

  #[test]
  fn deep_links_linux_adds_mimetype_and_url_field_code() {
    let tmp = tempfile::tempdir().unwrap();
    let path = fake_desktop_file(
      tmp.path(),
      "[Desktop Entry]\nName=MyApp\nExec=/opt/myapp/myapp\nType=Application\n",
    );
    register_deep_links_linux(
      tmp.path(),
      &["acme".to_string(), "acme-beta".to_string()],
    )
    .unwrap();

    let out = std::fs::read_to_string(&path).unwrap();
    // The launcher must receive the URL, so `%u` is appended exactly once.
    assert!(
      out.contains("Exec=/opt/myapp/myapp %u"),
      "Exec should gain a %u field code; got:\n{out}"
    );
    assert!(
      out
        .contains("MimeType=x-scheme-handler/acme;x-scheme-handler/acme-beta;"),
      "MimeType should list every scheme handler; got:\n{out}"
    );
  }

  #[test]
  fn deep_links_linux_does_not_duplicate_field_code() {
    let tmp = tempfile::tempdir().unwrap();
    let path = fake_desktop_file(
      tmp.path(),
      "[Desktop Entry]\nExec=/opt/myapp/myapp %U\nType=Application\n",
    );
    register_deep_links_linux(tmp.path(), &["acme".to_string()]).unwrap();

    let out = std::fs::read_to_string(&path).unwrap();
    // An existing `%U` (or `%u`) already forwards the URL — don't add another.
    assert!(
      out.contains("Exec=/opt/myapp/myapp %U\n"),
      "existing field code should be left untouched; got:\n{out}"
    );
    assert!(!out.contains("%U %u") && !out.contains("%u %U"));
  }

  #[test]
  fn deep_links_linux_merges_into_existing_mimetype() {
    let tmp = tempfile::tempdir().unwrap();
    let path = fake_desktop_file(
      tmp.path(),
      "[Desktop Entry]\nExec=/opt/myapp/myapp\nMimeType=text/html\n",
    );
    register_deep_links_linux(tmp.path(), &["acme".to_string()]).unwrap();

    let out = std::fs::read_to_string(&path).unwrap();
    assert!(
      out.contains("MimeType=text/html;x-scheme-handler/acme;"),
      "existing MimeType entries should be preserved; got:\n{out}"
    );
  }

  #[test]
  fn deep_links_windows_writes_registry_script() {
    let tmp = tempfile::tempdir().unwrap();
    // The launcher is `<bundle-dir-name>.exe`, so the bundle dir must be named
    // after the app.
    let bundle = tmp.path().join("MyApp");
    std::fs::create_dir_all(&bundle).unwrap();
    register_deep_links_windows(
      &bundle,
      &["acme".to_string(), "acme-beta".to_string()],
    )
    .unwrap();

    let script =
      std::fs::read_to_string(bundle.join("register-deep-links.bat")).unwrap();
    for scheme in &["acme", "acme-beta"] {
      assert!(
        script
          .contains(&format!("reg add \"HKCU\\Software\\Classes\\{scheme}\"")),
        "script should register {scheme}; got:\n{script}"
      );
    }
    // The protocol handler must point back at the renamed launcher exe.
    assert!(
      script.contains("MyApp.exe"),
      "handler should invoke the launcher; got:\n{script}"
    );
  }

  #[test]
  fn register_deep_links_lowercases_and_dispatches_by_target() {
    // Drives the top-level entry point with an explicit target so the test
    // exercises the macOS path on any host. Mixed-case input must be
    // normalized to lowercase before it reaches the plist.
    let tmp = tempfile::tempdir().unwrap();
    let bundle = fake_macos_bundle(tmp.path(), "com.acme.myapp");
    let mut flags = empty_desktop_flags();
    flags.target = Some("aarch64-apple-darwin".to_string());
    flags.deep_links = vec!["  ACME  ".to_string(), "".to_string()];
    register_deep_links(&bundle, &flags).unwrap();

    let dict: plist::Dictionary =
      plist::from_file(bundle.join("Contents/Info.plist")).unwrap();
    let schemes: Vec<&str> = dict
      .get("CFBundleURLTypes")
      .and_then(|v| v.as_array())
      .unwrap()[0]
      .as_dictionary()
      .unwrap()
      .get("CFBundleURLSchemes")
      .and_then(|v| v.as_array())
      .unwrap()
      .iter()
      .map(|v| v.as_string().unwrap())
      .collect();
    assert_eq!(schemes, ["acme"]);
  }

  #[test]
  fn register_deep_links_rejects_reserved_scheme() {
    let tmp = tempfile::tempdir().unwrap();
    let bundle = fake_macos_bundle(tmp.path(), "com.acme.myapp");
    let mut flags = empty_desktop_flags();
    flags.target = Some("aarch64-apple-darwin".to_string());
    flags.deep_links = vec!["https".to_string()];
    assert!(register_deep_links(&bundle, &flags).is_err());
  }

  #[test]
  fn reserve_app_dir_ok_when_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("app");
    reserve_app_dir(&app_dir).unwrap();
    assert!(!app_dir.exists());
  }

  #[test]
  fn reserve_app_dir_clears_empty_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("app");
    std::fs::create_dir(&app_dir).unwrap();
    reserve_app_dir(&app_dir).unwrap();
    assert!(!app_dir.exists());
  }

  #[test]
  fn reserve_app_dir_clears_previous_build() {
    // A directory carrying our marker is a prior `deno desktop` output and
    // may be replaced.
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("app");
    std::fs::create_dir(&app_dir).unwrap();
    std::fs::write(app_dir.join("laufey"), b"binary").unwrap();
    std::fs::write(app_dir.join(APP_DIR_MARKER), b"").unwrap();
    reserve_app_dir(&app_dir).unwrap();
    assert!(!app_dir.exists());
  }

  #[test]
  fn reserve_app_dir_clears_previous_macos_bundle() {
    // A `.app` bundle's marker lives under Contents/Resources/.
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("App.app");
    let resources = app_dir.join("Contents").join("Resources");
    std::fs::create_dir_all(&resources).unwrap();
    std::fs::write(resources.join(APP_DIR_MARKER), b"").unwrap();
    reserve_app_dir(&app_dir).unwrap();
    assert!(!app_dir.exists());
  }

  #[test]
  fn reserve_app_dir_refuses_user_data() {
    // The crux of issue #35510: an inferred app name collides with an
    // existing user directory. We must error, not delete it.
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("helloworld");
    std::fs::create_dir(&app_dir).unwrap();
    let precious = app_dir.join("precious.txt");
    std::fs::write(&precious, b"do not delete").unwrap();

    let err = reserve_app_dir(&app_dir).unwrap_err();
    assert!(err.to_string().contains("not created by"));
    // The user's data survives untouched.
    assert!(precious.exists());
    assert_eq!(std::fs::read(&precious).unwrap(), b"do not delete");
  }

  #[test]
  fn reserve_app_dir_refuses_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("app");
    std::fs::write(&path, b"i am a file").unwrap();
    let err = reserve_app_dir(&path).unwrap_err();
    assert!(err.to_string().contains("a file with that name"));
    assert!(path.exists());
  }

  // --- desktop.backend config merge (CLI flag > deno.json > webview) ---

  #[test]
  fn backend_from_deno_json_when_flag_absent() {
    let mut flags = DesktopFlags {
      source_file: "main.ts".to_string(),
      backend: None,
      ..Default::default()
    };
    let config = DesktopConfig {
      backend: Some("cef".to_string()),
      ..Default::default()
    };
    apply_desktop_config_to_flags(&mut flags, config);
    assert_eq!(flags.backend.as_deref(), Some("cef"));
  }

  #[test]
  fn cli_flag_overrides_deno_json_backend() {
    let mut flags = DesktopFlags {
      source_file: "main.ts".to_string(),
      backend: Some("webview".to_string()),
      ..Default::default()
    };
    let config = DesktopConfig {
      backend: Some("cef".to_string()),
      ..Default::default()
    };
    apply_desktop_config_to_flags(&mut flags, config);
    assert_eq!(flags.backend.as_deref(), Some("webview"));
  }

  #[test]
  fn backend_defaults_to_none_when_unset() {
    let mut flags = DesktopFlags {
      source_file: "main.ts".to_string(),
      backend: None,
      ..Default::default()
    };
    let config = DesktopConfig::default();
    apply_desktop_config_to_flags(&mut flags, config);
    // Left unset; callers fall back to "webview" via unwrap_or("webview").
    assert_eq!(flags.backend.as_deref(), None);
  }
}
