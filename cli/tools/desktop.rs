// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_terminal::colors;

use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::args::DenoSubcommand;
use crate::args::DesktopFlags;
use crate::args::Flags;
use crate::args::TypeCheckMode;
use crate::factory::CliFactory;

pub async fn desktop(
  flags: Flags,
  mut desktop_flags: DesktopFlags,
) -> Result<(), AnyError> {
  let all_targets = desktop_flags.all_targets;

  let config_flags = flags.clone();
  let factory = CliFactory::from_flags(Arc::new(config_flags));
  let cli_options = factory.cli_options()?;
  let desktop_config = cli_options.start_dir.to_desktop_config()?.clone();

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
      compile_desktop(flags.clone(), desktop_flags, cli_options).await?;
    }
    Ok(())
  } else {
    compile_desktop(flags, desktop_flags, cli_options).await
  }
}

async fn compile_desktop(
  mut flags: Flags,
  mut desktop_flags: DesktopFlags,
  cli_options: &Arc<CliOptions>,
) -> Result<(), AnyError> {
  // Desktop framework detection: when --desktop is used and the source is
  // "." (a directory), detect the framework and generate the entrypoint.
  let _desktop_entrypoint_file = if desktop_flags.source_file == "." {
    let cwd = flags
      .initial_cwd
      .clone()
      .unwrap_or_else(|| std::env::current_dir().unwrap());
    if let Some(detection) = super::framework::detect_framework(&cwd)? {
      let entrypoint_code = detection.entrypoint_code;
      let includes = detection.include_paths;
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
      // Write a temporary entrypoint file.
      let entrypoint_path = cwd.join(".deno_desktop_entry.ts");
      std::fs::write(&entrypoint_path, entrypoint_code)?;
      let entrypoint_str = entrypoint_path.display().to_string();
      desktop_flags.source_file = entrypoint_str.clone();
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
      Some(entrypoint_path)
    } else {
      bail!(
        "Could not detect a supported framework in the current directory.\nSupported frameworks: Next.js, Astro\nProvide an explicit entrypoint instead of \".\"."
      );
    }
  } else {
    None
  };

  let self_extracting = _desktop_entrypoint_file.is_some();

  // Clean up temp entrypoint on exit.
  struct CleanupGuard(Option<PathBuf>);
  impl Drop for CleanupGuard {
    fn drop(&mut self) {
      if let Some(ref path) = self.0 {
        let _ = std::fs::remove_file(path);
      }
    }
  }
  let _cleanup = CleanupGuard(_desktop_entrypoint_file);

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

  if desktop_flags.hmr {
    let cwd = cli_options.initial_cwd();
    let framework = super::framework::detect_framework(cwd)?;
    let backend = desktop_flags.backend.as_deref().unwrap_or("webview");
    run_desktop_hmr(&output_path, cwd, framework.as_ref(), backend).await?;
  } else {
    // Package the dylib into a platform-specific app bundle.
    let bundle_path =
      package_desktop_app(&output_path, &desktop_flags, cli_options)?;
    let initial_cwd =
      deno_path_util::url_from_directory_path(cli_options.initial_cwd())?;
    log::info!(
      "{} {}",
      colors::green("Bundle"),
      if let Ok(bundle_url) = deno_path_util::url_from_file_path(&bundle_path) {
        crate::util::path::relative_specifier_path_for_display(
          &initial_cwd,
          &bundle_url,
        )
      } else {
        bundle_path.display().to_string()
      }
    );
  }

  Ok(())
}

/// Launch the desktop app with HMR enabled after compilation.
///
/// The compiled dylib contains the entrypoint with both production and dev
/// code paths. The `dev_entrypoint_code` is parsed as KEY=VALUE env vars
/// that switch the entrypoint to dev mode (e.g. DENO_DESKTOP_DEV=1).
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
) -> Result<(), AnyError> {
  let wef_backend = find_wef_backend(backend)?;
  let dylib_abs = dylib_path
    .canonicalize()
    .unwrap_or(dylib_path.to_path_buf());
  let source_abs = source_dir
    .canonicalize()
    .unwrap_or(source_dir.to_path_buf());

  if let Some(fw) = framework {
    if fw.dev_entrypoint_code.is_some() {
      log::info!(
        "{} {} dev server with HMR in desktop mode",
        colors::green("Running"),
        fw.name,
      );
    }
  }

  log::info!(
    "{} desktop app with HMR (watching {})",
    colors::green("Running"),
    source_abs.display(),
  );

  let mut cmd = std::process::Command::new(&wef_backend);
  cmd
    .arg("--runtime")
    .arg(&dylib_abs)
    .env("WEF_RUNTIME_PATH", &dylib_abs)
    .env("DENO_DESKTOP_HMR", &source_abs)
    .current_dir(&source_abs);

  // Set framework dev env vars (e.g. DENO_DESKTOP_DEV=1) so the
  // embedded entrypoint branches into dev mode.
  if let Some(fw) = framework {
    if let Some(ref dev_env) = fw.dev_entrypoint_code {
      for line in dev_env.lines() {
        if let Some((key, value)) = line.split_once('=') {
          cmd.env(key.trim(), value.trim());
        }
      }
    }
  }

  let mut child = cmd.spawn().with_context(|| {
    format!("Failed to launch WEF backend: {}", wef_backend.display())
  })?;

  let status = child.wait().context("Failed waiting for WEF backend")?;

  if !status.success() {
    bail!("WEF backend exited with status: {}", status);
  }
  Ok(())
}

/// Package a compiled desktop dylib into a platform-specific app bundle.
fn package_desktop_app(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
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
    package_macos_app_bundle(dylib_path, desktop_flags, cli_options)
  } else if is_windows {
    package_windows_app_dir(dylib_path, desktop_flags, cli_options)
  } else {
    package_linux_app_dir(dylib_path, desktop_flags, cli_options)
  }
}

/// Find the directory containing the WEF backend binary. For non-macOS
/// platforms, the binary and its support files (e.g., CEF DLLs, locales)
/// sit alongside each other in a single flat directory.
fn find_wef_backend_dir(backend: &str) -> Result<PathBuf, AnyError> {
  let backend_binary = find_wef_backend(backend)?;
  let parent = backend_binary.parent().ok_or_else(|| {
    deno_core::anyhow::anyhow!(
      "WEF backend binary has no parent directory: {}",
      backend_binary.display()
    )
  })?;
  Ok(parent.to_path_buf())
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
fn package_windows_app_dir(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
) -> Result<PathBuf, AnyError> {
  let app_name = dylib_path
    .file_stem()
    .unwrap()
    .to_string_lossy()
    .to_string();
  let app_dir = dylib_path.parent().unwrap().join(&app_name);

  let backend = desktop_flags.backend.as_deref().unwrap_or("cef");
  let wef_dir = find_wef_backend_dir(backend)?;
  let wef_binary = find_wef_backend(backend)?;
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

  // Copy the compiled dylib (denort.dll) alongside the backend binary.
  let dylib_filename = dylib_path.file_name().unwrap();
  std::fs::copy(dylib_path, app_dir.join(dylib_filename))?;

  // Create a .bat launcher that invokes the backend with --runtime.
  let launcher_path = app_dir.join(format!("{}.bat", app_name));
  std::fs::write(
    &launcher_path,
    format!(
      "@echo off\r\n\
       set DIR=%~dp0\r\n\
       \"%DIR%{wef_binary}\" --runtime \"%DIR%{dylib}\" %*\r\n",
      wef_binary = wef_binary_name,
      dylib = dylib_filename.to_string_lossy(),
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
fn package_linux_app_dir(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
) -> Result<PathBuf, AnyError> {
  let app_name = dylib_path
    .file_stem()
    .unwrap()
    .to_string_lossy()
    .to_string();
  // `file_stem` on "libdenort.so" returns "libdenort" — strip the "lib" prefix
  // so the app directory is named after the app, not the runtime library.
  let app_name = app_name
    .strip_prefix("lib")
    .map(|s| s.to_string())
    .unwrap_or(app_name);
  let app_dir = dylib_path.parent().unwrap().join(&app_name);

  let backend = desktop_flags.backend.as_deref().unwrap_or("cef");
  let wef_dir = find_wef_backend_dir(backend)?;
  let wef_binary = find_wef_backend(backend)?;
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

  // Copy the compiled dylib alongside the backend binary.
  let dylib_filename = dylib_path.file_name().unwrap();
  std::fs::copy(dylib_path, app_dir.join(dylib_filename))?;

  // Create a shell launcher that invokes the backend with --runtime.
  let launcher_path = app_dir.join(&app_name);
  std::fs::write(
    &launcher_path,
    format!(
      "#!/bin/bash\n\
       DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
       exec \"$DIR/{wef_binary}\" --runtime \"$DIR/{dylib}\" \"$@\"\n",
      wef_binary = wef_binary_name,
      dylib = dylib_filename.to_string_lossy(),
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

/// Environment variable for the WEF backend executable path.
const WEF_BACKEND_ENV: &str = "WEF_BACKEND";

/// Find the deno repo root from the current exe path.
/// Dev builds live at `<repo>/target/{debug,release}/deno`.
/// Resolve WEF backend binary search paths based on the chosen backend.
fn wef_backend_search_paths(backend: &str) -> Vec<PathBuf> {
  let wef_base = option_env!("CARGO_MANIFEST_DIR")
    .map(|d| std::path::Path::new(d).join("../../wef"))
    .filter(|p| p.exists());

  let mut paths = Vec::new();
  if let Some(ref wef) = wef_base {
    match backend {
      "cef" => {
        paths
          .push(wef.join("result-cef/Applications/wef.app/Contents/MacOS/wef"));
        paths.push(wef.join("result/Applications/wef.app/Contents/MacOS/wef"));
        paths.push(wef.join("cef/build/Release/wef.app/Contents/MacOS/wef"));
        paths.push(wef.join("cef/build/wef.app/Contents/MacOS/wef"));
      }
      "servo" => {
        paths.push(wef.join("target/release/wef_servo"));
        paths.push(wef.join("target/debug/wef_servo"));
      }
      "raw" => {
        paths.push(wef.join("target/release/wef_winit"));
        paths.push(wef.join("target/debug/wef_winit"));
      }
      _ => {
        paths.push(wef.join(
          "result-1/Applications/wef_webview.app/Contents/MacOS/wef_webview",
        ));
        paths.push(wef.join(
          "result/Applications/wef_webview.app/Contents/MacOS/wef_webview",
        ));
        paths.push(
          wef.join("webview/build/wef_webview.app/Contents/MacOS/wef_webview"),
        );
      }
    }
  }
  paths
}

/// Environment variable pointing to a WEF backend .app bundle.
const WEF_BACKEND_APP_ENV: &str = "WEF_BACKEND_APP";

/// Resolve WEF backend .app search paths based on the chosen backend.
fn wef_backend_app_search_paths(backend: &str) -> Vec<String> {
  // Derive the wef repo path from this crate's location.
  // CARGO_MANIFEST_DIR is cli/, the wef repo is at ../../wef relative to that
  // (i.e. a sibling of the deno repo in the parent directory).
  let wef_base = option_env!("CARGO_MANIFEST_DIR")
    .map(|d| std::path::Path::new(d).join("../../wef"))
    .filter(|p| p.exists());

  let mut paths = Vec::new();
  if let Some(ref wef) = wef_base {
    match backend {
      "cef" => {
        paths.push(
          wef
            .join("result-cef/Applications/wef.app")
            .to_string_lossy()
            .to_string(),
        );
        paths.push(
          wef
            .join("result/Applications/wef.app")
            .to_string_lossy()
            .to_string(),
        );
        paths.push(
          wef
            .join("cef/build/Release/wef.app")
            .to_string_lossy()
            .to_string(),
        );
        paths.push(wef.join("cef/build/wef.app").to_string_lossy().to_string());
      }
      "raw" | "servo" => {} // Not .app bundles
      _ => {
        paths.push(
          wef
            .join("result-1/Applications/wef_webview.app")
            .to_string_lossy()
            .to_string(),
        );
        paths.push(
          wef
            .join("result/Applications/wef_webview.app")
            .to_string_lossy()
            .to_string(),
        );
        paths.push(
          wef
            .join("webview/build/wef_webview.app")
            .to_string_lossy()
            .to_string(),
        );
      }
    }
  }
  paths
}

/// Find the WEF backend .app bundle directory.
fn find_wef_backend_app_bundle(backend: &str) -> Result<PathBuf, AnyError> {
  // Check explicit env var first.
  if let Ok(path) = std::env::var(WEF_BACKEND_APP_ENV) {
    let p = PathBuf::from(&path);
    if p.exists() && p.extension().map_or(false, |e| e == "app") {
      return Ok(p);
    }
    bail!(
      "WEF backend .app not found at {} (set via {})",
      path,
      WEF_BACKEND_APP_ENV
    );
  }

  // Search well-known paths.
  let exe_dir = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|p| p.to_path_buf()));
  for search_path in wef_backend_app_search_paths(backend) {
    let p = PathBuf::from(&search_path);
    if p.exists() {
      return Ok(p);
    }
    if let Some(exe_dir) = &exe_dir {
      let p = exe_dir.join(&search_path);
      if p.exists() {
        return Ok(p);
      }
    }
  }

  // Derive from the WEF backend binary path (walk up to .app).
  if let Ok(backend_binary) = find_wef_backend(backend) {
    let mut current = backend_binary.as_path();
    while let Some(parent) = current.parent() {
      if parent.extension().map_or(false, |e| e == "app") {
        return Ok(parent.to_path_buf());
      }
      current = parent;
    }
  }

  bail!(
    "WEF backend '{}' .app bundle not found. Set {} to the path of the WEF .app bundle.",
    backend,
    WEF_BACKEND_APP_ENV
  )
}

/// Extract a string value from a plist XML by key.
fn extract_plist_string(plist_xml: &str, key: &str) -> Option<String> {
  let key_tag = format!("<key>{}</key>", key);
  let pos = plist_xml.find(&key_tag)?;
  let after_key = &plist_xml[pos + key_tag.len()..];
  let start = after_key.find("<string>")? + "<string>".len();
  let end = after_key.find("</string>")?;
  Some(after_key[start..end].to_string())
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
fn package_macos_app_bundle(
  dylib_path: &Path,
  desktop_flags: &DesktopFlags,
  cli_options: &CliOptions,
) -> Result<PathBuf, AnyError> {
  let app_name = dylib_path
    .file_stem()
    .unwrap()
    .to_string_lossy()
    .to_string();
  let app_bundle = dylib_path
    .parent()
    .unwrap()
    .join(format!("{}.app", app_name));

  // Find the WEF backend .app and its main executable.
  let backend = desktop_flags.backend.as_deref().unwrap_or("cef");
  let wef_app = find_wef_backend_app_bundle(backend)?;
  let wef_plist_path = wef_app.join("Contents/Info.plist");
  let wef_executable_name = if wef_plist_path.exists() {
    let plist_content = std::fs::read_to_string(&wef_plist_path)?;
    extract_plist_string(&plist_content, "CFBundleExecutable")
      .unwrap_or_else(|| "wef_webview".to_string())
  } else {
    "wef_webview".to_string()
  };
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

  // Strip unnecessary bulk from the CEF framework.
  strip_cef_bloat(&contents_dir);

  // Copy the compiled dylib.
  let dylib_filename = dylib_path.file_name().unwrap();
  std::fs::copy(dylib_path, macos_dir.join(dylib_filename))?;

  // Create launcher script as the main executable.
  let launcher_path = macos_dir.join(&app_name);
  std::fs::write(
    &launcher_path,
    format!(
      "#!/bin/bash\n\
       DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
       exec \"$DIR/{wef_binary}\" --runtime \"$DIR/{dylib}\" \"$@\"\n",
      wef_binary = wef_executable_name,
      dylib = dylib_filename.to_string_lossy(),
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

/// Find the WEF backend executable.
fn find_wef_backend(backend: &str) -> Result<PathBuf, AnyError> {
  if let Ok(path) = std::env::var(WEF_BACKEND_ENV) {
    let p = PathBuf::from(&path);
    if p.exists() {
      return Ok(p);
    }
    bail!(
      "WEF backend not found at {} (set via {})",
      path,
      WEF_BACKEND_ENV
    );
  }

  // Search relative to the deno executable for development.
  let exe_dir = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|p| p.to_path_buf()));
  for search_path in wef_backend_search_paths(backend) {
    let p = PathBuf::from(&search_path);
    if p.exists() {
      return Ok(p);
    }
    if let Some(exe_dir) = &exe_dir {
      let p = exe_dir.join(&search_path);
      if p.exists() {
        return Ok(p);
      }
    }
  }

  bail!(
    "WEF backend '{}' not found. Set {} to the path of the WEF backend executable.",
    backend,
    WEF_BACKEND_ENV
  )
}

/// Build a macOS `.icns` from an icon set (multiple PNGs at specified sizes).
///
/// Maps each provided size to the correct `.iconset` filename. The standard
/// macOS iconset uses 1x and 2x variants:
///   16px  → icon_16x16.png
///   32px  → icon_16x16@2x.png AND icon_32x32.png
///   64px  → icon_32x32@2x.png
///   128px → icon_128x128.png
///   256px → icon_128x128@2x.png AND icon_256x256.png
///   512px → icon_256x256@2x.png AND icon_512x512.png
///   1024px→ icon_512x512@2x.png
fn convert_icon_set_to_icns(
  cwd: &Path,
  entries: &[crate::args::IconSetEntry],
  icns_path: &Path,
) -> Result<(), deno_core::error::AnyError> {
  let iconset_dir = icns_path.with_extension("iconset");
  std::fs::create_dir_all(&iconset_dir)?;

  for entry in entries {
    let src = cwd.join(&entry.path);
    if !src.exists() {
      log::warn!("Icon '{}' not found, skipping", src.display());
      continue;
    }

    let names = iconset_names_for_size(entry.size);
    for name in names {
      std::fs::copy(&src, iconset_dir.join(name))?;
    }
  }

  let status = std::process::Command::new("iconutil")
    .args([
      "-c",
      "icns",
      &iconset_dir.display().to_string(),
      "-o",
      &icns_path.display().to_string(),
    ])
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status()?;

  let _ = std::fs::remove_dir_all(&iconset_dir);

  if !status.success() {
    deno_core::anyhow::bail!("iconutil failed to create .icns from icon set");
  }

  Ok(())
}

/// Returns the `.iconset` filenames a given pixel size maps to.
fn iconset_names_for_size(size: u32) -> Vec<&'static str> {
  match size {
    16 => vec!["icon_16x16.png"],
    32 => vec!["icon_16x16@2x.png", "icon_32x32.png"],
    64 => vec!["icon_32x32@2x.png"],
    128 => vec!["icon_128x128.png"],
    256 => vec!["icon_128x128@2x.png", "icon_256x256.png"],
    512 => vec!["icon_256x256@2x.png", "icon_512x512.png"],
    1024 => vec!["icon_512x512@2x.png"],
    _ => {
      log::warn!(
        "Icon size {}px doesn't map to a standard macOS iconset slot, skipping",
        size
      );
      vec![]
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
