// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json::NodeModulesDirMode;
use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_graph::GraphKind;
use deno_npm_installer::graph::NpmCachingStrategy;
use deno_path_util::resolve_url_or_path;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_terminal::colors;
use rand::Rng;

use super::installer::BinNameResolver;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::args::ConfigFlag;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::standalone::binary::WriteBinOptions;
use crate::standalone::binary::is_standalone_binary;
use crate::util::temp::create_temp_node_modules_dir;

/// Environment variable for the WEF backend executable path.
const WEF_BACKEND_ENV: &str = "WEF_BACKEND";

/// Find the deno repo root from the current exe path.
/// Dev builds live at `<repo>/target/{debug,release}/deno`.
/// Resolve WEF backend binary search paths based on the chosen backend.
fn wef_backend_search_paths(backend: &str) -> Vec<PathBuf> {
  match backend {
    "cef" => vec![
      PathBuf::from("/Users/divy/gh/wef/result/Applications/wef.app/Contents/MacOS/wef"),
      PathBuf::from("/Users/divy/gh/wef/cef/build/wef.app/Contents/MacOS/wef"),
    ],
    "servo" => vec![
      PathBuf::from("/Users/divy/gh/wef/servo/target/release/wef_servo"),
      PathBuf::from("/Users/divy/gh/wef/servo/target/debug/wef_servo"),
    ],
    _ => vec![
      PathBuf::from("/Users/divy/gh/wef/result-1/Applications/wef_webview.app/Contents/MacOS/wef_webview"),
      PathBuf::from("/Users/divy/gh/wef/result/Applications/wef_webview.app/Contents/MacOS/wef_webview"),
      PathBuf::from("/Users/divy/gh/wef/webview/build/wef_webview.app/Contents/MacOS/wef_webview"),
    ],
  }
}

pub async fn compile(
  mut flags: Flags,
  mut compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  // Desktop framework detection: when --desktop is used and the source is
  // "." (a directory), detect the framework and generate the entrypoint.
  let _desktop_entrypoint_file = if compile_flags.desktop
    && compile_flags.source_file == "."
  {
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
      // Write a temporary entrypoint file.
      let entrypoint_path = cwd.join(".deno_desktop_entry.ts");
      std::fs::write(&entrypoint_path, entrypoint_code)?;
      let entrypoint_str = entrypoint_path.display().to_string();
      compile_flags.source_file = entrypoint_str.clone();
      // Also update the source in flags.subcommand so resolve_main_module sees it.
      if let crate::args::DenoSubcommand::Compile(ref mut cf) = flags.subcommand
      {
        cf.source_file = entrypoint_str;
        cf.self_extracting = true;
        // Use the project directory name as the output binary name.
        if cf.output.is_none() {
          if let Some(dir_name) = cwd.file_name() {
            cf.output = Some(dir_name.to_string_lossy().into_owned());
          }
        }
      }
      if compile_flags.output.is_none() {
        if let Some(dir_name) = cwd.file_name() {
          compile_flags.output = Some(dir_name.to_string_lossy().into_owned());
        }
      }
      // Auto-enable self-extracting for framework apps.
      compile_flags.self_extracting = true;
      // Add framework build output to includes.
      for inc in includes {
        if !compile_flags.include.contains(&inc) {
          compile_flags.include.push(inc.clone());
        }
        if let crate::args::DenoSubcommand::Compile(ref mut cf) =
          flags.subcommand
        {
          if !cf.include.contains(&inc) {
            cf.include.push(inc);
          }
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

  // use a temporary directory with a node_modules folder when the user
  // specifies an npm package for better compatibility
  let _temp_dir =
    if compile_flags.source_file.to_lowercase().starts_with("npm:")
      && flags.node_modules_dir.is_none()
      && !matches!(flags.config_flag, ConfigFlag::Path(_))
    {
      let temp_node_modules_dir = create_temp_node_modules_dir()
        .context("Failed creating temp directory for node_modules folder.")?;
      flags.initial_cwd = Some(temp_node_modules_dir.parent().to_path_buf());
      flags.internal.root_node_modules_dir_override =
        Some(temp_node_modules_dir.node_modules_dir_path().to_path_buf());
      flags.node_modules_dir = Some(NodeModulesDirMode::Auto);
      Some(temp_node_modules_dir)
    } else {
      None
    };
  let flags = Arc::new(flags);
  // boxed_local() is to avoid large futures
  if compile_flags.eszip {
    compile_eszip(flags, compile_flags).boxed_local().await
  } else {
    compile_binary(flags, compile_flags).boxed_local().await
  }
}

async fn compile_binary(
  flags: Arc<Flags>,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let module_graph_creator = factory.module_graph_creator().await?;
  let binary_writer = factory.create_compile_binary_writer().await?;
  let entrypoint = cli_options.resolve_main_module()?;
  let bin_name_resolver = factory.bin_name_resolver()?;
  let output_path = resolve_compile_executable_output_path(
    &bin_name_resolver,
    &compile_flags,
    cli_options.initial_cwd(),
  )
  .await?;
  let (module_roots, include_paths) = get_module_roots_and_include_paths(
    entrypoint,
    &compile_flags,
    cli_options,
  )?;

  let graph = Arc::try_unwrap(
    module_graph_creator
      .create_graph_and_maybe_check(module_roots.clone())
      .await?,
  )
  .unwrap();
  let graph = if cli_options.type_check_mode().is_true() {
    // In this case, the previous graph creation did type checking, which will
    // create a module graph with types information in it. We don't want to
    // store that in the binary so create a code only module graph from scratch.
    module_graph_creator
      .create_graph(
        GraphKind::CodeOnly,
        module_roots,
        NpmCachingStrategy::Eager,
      )
      .await?
  } else {
    graph
  };

  let initial_cwd =
    deno_path_util::url_from_directory_path(cli_options.initial_cwd())?;

  log::info!(
    "{} {} to {}",
    colors::green("Compile"),
    crate::util::path::relative_specifier_path_for_display(
      &initial_cwd,
      entrypoint
    ),
    {
      if let Ok(output_path) = deno_path_util::url_from_file_path(&output_path)
      {
        crate::util::path::relative_specifier_path_for_display(
          &initial_cwd,
          &output_path,
        )
      } else {
        output_path.display().to_string()
      }
    }
  );
  validate_output_path(&output_path)?;

  let mut temp_filename = output_path.file_name().unwrap().to_owned();
  temp_filename.push(format!(
    ".tmp-{}",
    faster_hex::hex_encode(
      &rand::thread_rng().r#gen::<[u8; 8]>(),
      &mut [0u8; 16]
    )
    .unwrap()
  ));
  let temp_path = output_path.with_file_name(temp_filename);

  let file = std::fs::File::create(&temp_path).with_context(|| {
    format!("Opening temporary file '{}'", temp_path.display())
  })?;

  let write_result = binary_writer
    .write_bin(WriteBinOptions {
      writer: file,
      display_output_filename: &output_path
        .file_name()
        .unwrap()
        .to_string_lossy(),
      graph: &graph,
      entrypoint,
      include_paths: &include_paths,
      exclude_paths: compile_flags
        .exclude
        .iter()
        .map(|p| cli_options.initial_cwd().join(p))
        .chain(std::iter::once(
          cli_options.initial_cwd().join(&output_path),
        ))
        .chain(std::iter::once(cli_options.initial_cwd().join(&temp_path)))
        .collect(),
      compile_flags: &compile_flags,
    })
    .await
    .with_context(|| {
      format!(
        "Writing deno compile executable to temporary file '{}'",
        temp_path.display()
      )
    });

  // set it as executable
  #[cfg(unix)]
  let write_result = write_result.and_then(|_| {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&temp_path, perms).with_context(|| {
      format!(
        "Setting permissions on temporary file '{}'",
        temp_path.display()
      )
    })
  });

  let write_result = write_result.and_then(|_| {
    std::fs::rename(&temp_path, &output_path).with_context(|| {
      format!(
        "Renaming temporary file '{}' to '{}'",
        temp_path.display(),
        output_path.display()
      )
    })
  });

  if let Err(err) = write_result {
    // errored, so attempt to remove the temporary file
    let _ = std::fs::remove_file(temp_path);
    return Err(err);
  }

  // When --desktop --hmr, compile and immediately run with HMR enabled.
  if compile_flags.desktop && compile_flags.hmr {
    let cwd = cli_options.initial_cwd();
    let framework = super::framework::detect_framework(cwd)?;
    let backend = compile_flags.backend.as_deref().unwrap_or("webview");
    run_desktop_hmr(&output_path, cwd, framework.as_ref(), backend)
      .await?;
  } else if compile_flags.desktop {
    // Package the dylib into a platform-specific app bundle.
    let bundle_path =
      package_desktop_app(&output_path, &compile_flags, cli_options)?;
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

/// Package a compiled desktop dylib into a platform-specific app bundle.
fn package_desktop_app(
  dylib_path: &Path,
  compile_flags: &CompileFlags,
  cli_options: &CliOptions,
) -> Result<PathBuf, AnyError> {
  let is_darwin = match &compile_flags.target {
    Some(target) => target.contains("darwin"),
    None => cfg!(target_os = "macos"),
  };

  if is_darwin {
    package_macos_app_bundle(dylib_path, compile_flags, cli_options)
  } else {
    // TODO(divy): Windows and Linux packaging
    Ok(dylib_path.to_path_buf())
  }
}

/// Environment variable pointing to a WEF backend .app bundle.
const WEF_BACKEND_APP_ENV: &str = "WEF_BACKEND_APP";

/// Resolve WEF backend .app search paths based on the chosen backend.
fn wef_backend_app_search_paths(backend: &str) -> Vec<String> {
  match backend {
    "cef" => vec![
      "/Users/divy/gh/wef/result/Applications/wef.app".to_string(),
      "/Users/divy/gh/wef/cef/build/wef.app".to_string(),
    ],
    "servo" => vec![], // Servo is not an .app bundle
    _ => vec![
      "/Users/divy/gh/wef/result-1/Applications/wef_webview.app".to_string(),
      "/Users/divy/gh/wef/result/Applications/wef_webview.app".to_string(),
      "/Users/divy/gh/wef/webview/build/wef_webview.app".to_string(),
    ],
  }
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
  compile_flags: &CompileFlags,
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
  let backend = compile_flags.backend.as_deref().unwrap_or("webview");
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
  copy_dir_all(&wef_app, &app_bundle)?;

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
  let has_icon = compile_flags.icon.is_some();
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
</dict>
</plist>
"#,
    app_name = app_name,
    bundle_id = bundle_id,
    icon_file = if has_icon { "AppIcon" } else { "" },
  );
  std::fs::write(contents_dir.join("Info.plist"), info_plist)?;

  // Handle icon.
  if let Some(ref icon) = compile_flags.icon {
    let icon_path = cli_options.initial_cwd().join(icon);
    if icon_path.exists() {
      let dest = resources_dir.join("AppIcon.icns");
      match icon_path.extension().and_then(|e| e.to_str()) {
        Some("icns") => {
          std::fs::copy(&icon_path, &dest)?;
        }
        Some("png") => {
          convert_png_to_icns(&icon_path, &dest)?;
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

  // Remove the standalone dylib (it's now inside the .app).
  let _ = std::fs::remove_file(dylib_path);

  Ok(app_bundle)
}

/// Convert a PNG image to macOS .icns format using `sips` and `iconutil`.
fn convert_png_to_icns(
  png_path: &Path,
  icns_path: &Path,
) -> Result<(), AnyError> {
  let iconset_dir = icns_path.with_extension("iconset");
  std::fs::create_dir_all(&iconset_dir)?;

  let sizes: &[(u32, &str)] = &[
    (16, "icon_16x16.png"),
    (32, "icon_16x16@2x.png"),
    (32, "icon_32x32.png"),
    (64, "icon_32x32@2x.png"),
    (128, "icon_128x128.png"),
    (256, "icon_128x128@2x.png"),
    (256, "icon_256x256.png"),
    (512, "icon_256x256@2x.png"),
    (512, "icon_512x512.png"),
    (1024, "icon_512x512@2x.png"),
  ];

  for (size, name) in sizes {
    let dest = iconset_dir.join(name);
    let status = std::process::Command::new("sips")
      .args([
        "-z",
        &size.to_string(),
        &size.to_string(),
        &png_path.display().to_string(),
        "--out",
        &dest.display().to_string(),
      ])
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status();
    if status.map_or(true, |s| !s.success()) {
      std::fs::copy(png_path, &dest)?;
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
    bail!("Failed to convert PNG to ICNS. Provide an .icns file directly or ensure iconutil is available.");
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

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), AnyError> {
  std::fs::create_dir_all(dst)?;
  for entry in std::fs::read_dir(src).with_context(|| {
    format!("Reading directory '{}'", src.display())
  })? {
    let entry = entry?;
    let ty = entry.file_type()?;
    let dest = dst.join(entry.file_name());
    if ty.is_dir() {
      copy_dir_all(&entry.path(), &dest)?;
    } else if ty.is_symlink() {
      let target = std::fs::read_link(entry.path())?;
      #[cfg(unix)]
      std::os::unix::fs::symlink(&target, &dest)?;
      #[cfg(windows)]
      {
        if target.is_dir() {
          std::os::windows::fs::symlink_dir(&target, &dest)?;
        } else {
          std::os::windows::fs::symlink_file(&target, &dest)?;
        }
      }
    } else {
      std::fs::copy(entry.path(), &dest)?;
      // Ensure the copied file is writable (nix store files are read-only).
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&dest)?;
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o200);
        std::fs::set_permissions(&dest, perms)?;
      }
    }
  }
  Ok(())
}

async fn compile_eszip(
  flags: Arc<Flags>,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let module_graph_creator = factory.module_graph_creator().await?;
  let parsed_source_cache = factory.parsed_source_cache()?;
  let compiler_options_resolver = factory.compiler_options_resolver()?;
  let bin_name_resolver = factory.bin_name_resolver()?;
  let entrypoint = cli_options.resolve_main_module()?;
  let mut output_path = resolve_compile_executable_output_path(
    &bin_name_resolver,
    &compile_flags,
    cli_options.initial_cwd(),
  )
  .await?;
  output_path.set_extension("eszip");

  let maybe_import_map_specifier =
    cli_options.resolve_specified_import_map_specifier()?;
  let (module_roots, _include_paths) = get_module_roots_and_include_paths(
    entrypoint,
    &compile_flags,
    cli_options,
  )?;

  let graph = Arc::try_unwrap(
    module_graph_creator
      .create_graph_and_maybe_check(module_roots.clone())
      .await?,
  )
  .unwrap();
  let graph = if cli_options.type_check_mode().is_true() {
    // In this case, the previous graph creation did type checking, which will
    // create a module graph with types information in it. We don't want to
    // store that in the binary so create a code only module graph from scratch.
    module_graph_creator
      .create_graph(
        GraphKind::CodeOnly,
        module_roots,
        NpmCachingStrategy::Eager,
      )
      .await?
  } else {
    graph
  };

  let transpile_and_emit_options = compiler_options_resolver
    .for_specifier(cli_options.workspace().root_dir_url())
    .transpile_options()?;
  let transpile_options = transpile_and_emit_options.transpile.clone();
  let emit_options = transpile_and_emit_options.emit.clone();

  let parser = parsed_source_cache.as_capturing_parser();
  let root_dir_url = cli_options.workspace().root_dir_url();
  log::debug!("Binary root dir: {}", root_dir_url);
  let relative_file_base = eszip::EszipRelativeFileBaseUrl::new(root_dir_url);
  let mut eszip = eszip::EszipV2::from_graph(eszip::FromGraphOptions {
    graph,
    parser,
    transpile_options,
    emit_options,
    relative_file_base: Some(relative_file_base),
    npm_packages: None,
    module_kind_resolver: Default::default(),
    npm_snapshot: Default::default(),
  })?;

  if let Some(import_map_specifier) = maybe_import_map_specifier {
    let import_map_path = import_map_specifier.to_file_path().unwrap();
    let import_map_content = std::fs::read_to_string(&import_map_path)
      .with_context(|| {
        format!("Failed to read import map: {:?}", import_map_path)
      })?;

    let import_map_specifier_str = if let Some(relative_import_map_specifier) =
      root_dir_url.make_relative(&import_map_specifier)
    {
      relative_import_map_specifier
    } else {
      import_map_specifier.to_string()
    };

    eszip.add_import_map(
      eszip::ModuleKind::Json,
      import_map_specifier_str,
      import_map_content.as_bytes().to_vec().into(),
    );
  }

  log::info!(
    "{} {} to {}",
    colors::green("Compile"),
    entrypoint,
    output_path.display(),
  );
  validate_output_path(&output_path)?;

  let mut file = std::fs::File::create(&output_path).with_context(|| {
    format!("Opening ESZip file '{}'", output_path.display())
  })?;

  let write_result = {
    let r = file.write_all(&eszip.into_bytes());
    drop(file);
    r
  };

  if let Err(err) = write_result {
    let _ = std::fs::remove_file(output_path);
    return Err(err.into());
  }

  Ok(())
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

/// This function writes out a final binary to specified path. If output path
/// is not already standalone binary it will return error instead.
fn validate_output_path(output_path: &Path) -> Result<(), AnyError> {
  if output_path.exists() {
    // If the output is a directory, throw error
    if output_path.is_dir() {
      bail!(
        concat!(
          "Could not compile to file '{}' because a directory exists with ",
          "the same name. You can use the `--output <file-path>` flag to ",
          "provide an alternative name."
        ),
        output_path.display()
      );
    }

    // Make sure we don't overwrite any file not created by Deno compiler because
    // this filename is chosen automatically in some cases.
    if !is_standalone_binary(output_path) {
      bail!(
        concat!(
          "Could not compile to file '{}' because the file already exists ",
          "and cannot be overwritten. Please delete the existing file or ",
          "use the `--output <file-path>` flag to provide an alternative name."
        ),
        output_path.display()
      );
    }

    // Remove file if it was indeed a deno compiled binary, to avoid corruption
    // (see https://github.com/denoland/deno/issues/10310)
    std::fs::remove_file(output_path)?;
  } else {
    let output_base = &output_path.parent().unwrap();
    if output_base.exists() && output_base.is_file() {
      bail!(
        concat!(
          "Could not compile to file '{}' because its parent directory ",
          "is an existing file. You can use the `--output <file-path>` flag to ",
          "provide an alternative name.",
        ),
        output_base.display(),
      );
    }
    std::fs::create_dir_all(output_base)?;
  }

  Ok(())
}

fn get_module_roots_and_include_paths(
  entrypoint: &ModuleSpecifier,
  compile_flags: &CompileFlags,
  cli_options: &Arc<CliOptions>,
) -> Result<(Vec<ModuleSpecifier>, Vec<ModuleSpecifier>), AnyError> {
  let initial_cwd = cli_options.initial_cwd();

  fn is_module_graph_module(url: &ModuleSpecifier) -> bool {
    if url.scheme() != "file" {
      return true;
    }
    is_module_graph_media_type(MediaType::from_specifier(url))
  }

  fn is_module_graph_media_type(media_type: MediaType) -> bool {
    match media_type {
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx
      | MediaType::Json
      | MediaType::Wasm => true,
      MediaType::Css
      | MediaType::Html
      | MediaType::Jsonc
      | MediaType::Json5
      | MediaType::Markdown
      | MediaType::SourceMap
      | MediaType::Sql
      | MediaType::Unknown => false,
    }
  }

  fn analyze_path(
    url: &ModuleSpecifier,
    excluded_paths: &HashSet<PathBuf>,
    searched_paths: &mut HashSet<PathBuf>,
    mut add_path: impl FnMut(&Path),
  ) -> Result<(), AnyError> {
    let Ok(path) = url_to_file_path(url) else {
      return Ok(());
    };
    let mut pending = VecDeque::from([path]);
    while let Some(path) = pending.pop_front() {
      if !searched_paths.insert(path.clone()) {
        continue;
      }
      if excluded_paths.contains(&path) {
        continue;
      }
      if !path.is_dir() {
        add_path(&path);
        continue;
      }
      for entry in std::fs::read_dir(&path).with_context(|| {
        format!("Failed reading directory '{}'", path.display())
      })? {
        let entry = entry.with_context(|| {
          format!("Failed reading entry in directory '{}'", path.display())
        })?;
        pending.push_back(entry.path());
      }
    }
    Ok(())
  }

  let mut searched_paths = HashSet::new();
  let mut module_roots = Vec::new();
  let mut include_paths = Vec::new();
  let exclude_set = compile_flags
    .exclude
    .iter()
    .map(|path| initial_cwd.join(path))
    .collect::<HashSet<_>>();
  module_roots.push(entrypoint.clone());
  for side_module in &compile_flags.include {
    let url = resolve_url_or_path(side_module, initial_cwd)?;
    if is_module_graph_module(&url) {
      module_roots.push(url.clone());
    } else {
      analyze_path(&url, &exclude_set, &mut searched_paths, |file_path| {
        let media_type = MediaType::from_path(file_path);
        if is_module_graph_media_type(media_type)
          && let Ok(file_url) = url_from_file_path(file_path)
        {
          module_roots.push(file_url);
        }
      })?;
    }
    if url.scheme() == "file" {
      include_paths.push(url);
    }
  }

  for preload_module in cli_options.preload_modules()? {
    module_roots.push(preload_module);
  }

  for require_module in cli_options.require_modules()? {
    module_roots.push(require_module);
  }

  Ok((module_roots, include_paths))
}

async fn resolve_compile_executable_output_path(
  bin_name_resolver: &BinNameResolver<'_>,
  compile_flags: &CompileFlags,
  current_dir: &Path,
) -> Result<PathBuf, AnyError> {
  let module_specifier =
    resolve_url_or_path(&compile_flags.source_file, current_dir)?;

  let output_flag = compile_flags.output.clone();
  let mut output_path = if let Some(out) = output_flag.as_ref() {
    let mut out_path = PathBuf::from(out);
    if out.ends_with('/') || out.ends_with('\\') {
      if let Some(infer_file_name) = bin_name_resolver
        .infer_name_from_url(&module_specifier)
        .await
        .map(PathBuf::from)
      {
        out_path = out_path.join(infer_file_name);
      }
    } else {
      out_path = out_path.to_path_buf();
    }
    Some(out_path)
  } else {
    None
  };

  if output_flag.is_none() {
    output_path = bin_name_resolver
      .infer_name_from_url(&module_specifier)
      .await
      .map(PathBuf::from)
  }

  output_path.ok_or_else(|| anyhow!(
    "An executable name was not provided. One could not be inferred from the URL. Aborting.",
  )).map(|output_path| {
    if compile_flags.desktop {
      get_desktop_specific_filepath(output_path, &compile_flags.target)
    } else {
      get_os_specific_filepath(output_path, &compile_flags.target)
    }
  })
}

fn get_desktop_specific_filepath(
  output: PathBuf,
  target: &Option<String>,
) -> PathBuf {
  let is_windows = match target {
    Some(target) => target.contains("windows"),
    None => cfg!(windows),
  };
  let is_darwin = match target {
    Some(target) => target.contains("darwin"),
    None => cfg!(target_os = "macos"),
  };
  if is_windows {
    output.with_extension("dll")
  } else if is_darwin {
    output.with_extension("dylib")
  } else {
    output.with_extension("so")
  }
}

fn get_os_specific_filepath(
  output: PathBuf,
  target: &Option<String>,
) -> PathBuf {
  let is_windows = match target {
    Some(target) => target.contains("windows"),
    None => cfg!(windows),
  };
  if is_windows && output.extension().unwrap_or_default() != "exe" {
    if let Some(ext) = output.extension() {
      // keep version in my-exe-0.1.0 -> my-exe-0.1.0.exe
      output.with_extension(format!("{}.exe", ext.to_string_lossy()))
    } else {
      output.with_extension("exe")
    }
  } else {
    output
  }
}

#[cfg(test)]
mod test {
  use deno_npm::registry::TestNpmRegistryApi;
  use deno_npm::resolution::NpmVersionResolver;

  pub use super::*;
  use crate::http_util::HttpClientProvider;
  use crate::util::env::resolve_cwd;

  #[tokio::test]
  async fn resolve_compile_executable_output_path_target_linux() {
    let http_client = HttpClientProvider::new(None, None);
    let npm_api = TestNpmRegistryApi::default();
    let npm_version_resolver = NpmVersionResolver::default();
    let bin_name_resolver =
      BinNameResolver::new(&http_client, &npm_api, &npm_version_resolver);
    let path = resolve_compile_executable_output_path(
      &bin_name_resolver,
      &CompileFlags {
        source_file: "mod.ts".to_string(),
        output: Some(String::from("./file")),
        args: Vec::new(),
        target: Some("x86_64-unknown-linux-gnu".to_string()),
        no_terminal: false,
        icon: None,
        include: Default::default(),
        exclude: Default::default(),
        eszip: true,
        self_extracting: false,
        desktop: false,
        hmr: false,
        backend: None,
      },
      &resolve_cwd(None).unwrap(),
    )
    .await
    .unwrap();

    // no extension, no matter what the operating system is
    // because the target was specified as linux
    // https://github.com/denoland/deno/issues/9667
    assert_eq!(path.file_name().unwrap(), "file");
  }

  #[tokio::test]
  async fn resolve_compile_executable_output_path_target_windows() {
    let http_client = HttpClientProvider::new(None, None);
    let npm_api = TestNpmRegistryApi::default();
    let npm_version_resolver = NpmVersionResolver::default();
    let bin_name_resolver =
      BinNameResolver::new(&http_client, &npm_api, &npm_version_resolver);
    let path = resolve_compile_executable_output_path(
      &bin_name_resolver,
      &CompileFlags {
        source_file: "mod.ts".to_string(),
        output: Some(String::from("./file")),
        args: Vec::new(),
        target: Some("x86_64-pc-windows-msvc".to_string()),
        include: Default::default(),
        exclude: Default::default(),
        icon: None,
        no_terminal: false,
        eszip: true,
        self_extracting: false,
        desktop: false,
        hmr: false,
        backend: None,
      },
      &resolve_cwd(None).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "file.exe");
  }

  #[test]
  fn test_os_specific_file_path() {
    fn run_test(path: &str, target: Option<&str>, expected: &str) {
      assert_eq!(
        get_os_specific_filepath(
          PathBuf::from(path),
          &target.map(|s| s.to_string())
        ),
        PathBuf::from(expected)
      );
    }

    if cfg!(windows) {
      run_test("C:\\my-exe", None, "C:\\my-exe.exe");
      run_test("C:\\my-exe.exe", None, "C:\\my-exe.exe");
      run_test("C:\\my-exe-0.1.2", None, "C:\\my-exe-0.1.2.exe");
    } else {
      run_test("my-exe", Some("linux"), "my-exe");
      run_test("my-exe-0.1.2", Some("linux"), "my-exe-0.1.2");
    }

    run_test("C:\\my-exe", Some("windows"), "C:\\my-exe.exe");
    run_test("C:\\my-exe.exe", Some("windows"), "C:\\my-exe.exe");
    run_test("C:\\my-exe.0.1.2", Some("windows"), "C:\\my-exe.0.1.2.exe");
    run_test("my-exe-0.1.2", Some("linux"), "my-exe-0.1.2");
  }
}
