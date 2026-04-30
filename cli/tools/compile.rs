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
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::TypeCheckMode;
use crate::factory::CliFactory;
use crate::standalone::binary::WriteBinOptions;
use crate::standalone::binary::is_standalone_binary;
use crate::util::temp::create_temp_node_modules_dir;

pub async fn compile(
  mut flags: Flags,
  mut compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  // Framework detection: when the source is a directory, detect the
  // framework and generate an entrypoint automatically.
  let source_dir = if compile_flags.source_file == "." {
    Some(flags.initial_cwd.clone().unwrap_or_else(|| {
      crate::util::env::resolve_cwd(None).unwrap().to_path_buf()
    }))
  } else {
    let path = PathBuf::from(&compile_flags.source_file);
    let path = if path.is_absolute() {
      path
    } else {
      flags
        .initial_cwd
        .clone()
        .unwrap_or_else(|| {
          crate::util::env::resolve_cwd(None).unwrap().to_path_buf()
        })
        .join(path)
    };
    path.is_dir().then_some(path)
  };

  let _framework_entrypoint_file = if let Some(dir) = source_dir {
    if let Some(detection) = super::framework::detect_framework(&dir)? {
      log::info!("Detected {} framework", detection.name);
      // Run the framework's build step if needed.
      if let Some(build_cmd) = &detection.build_command {
        log::info!(
          "{} {} project...",
          colors::green("Building"),
          detection.name,
        );
        let status = std::process::Command::new(&build_cmd[0])
          .args(&build_cmd[1..])
          .current_dir(&dir)
          .status()
          .with_context(|| {
            format!("Failed to run build command: {}", build_cmd.join(" "))
          })?;
        if !status.success() {
          bail!(
            "{} build failed (exit code: {})",
            detection.name,
            status.code().unwrap_or(-1)
          );
        }
      }
      // Enable CJS detection for Node-based frameworks.
      flags.unstable_config.detect_cjs = true;
      if detection.name == "Next.js"
        && !matches!(flags.type_check_mode, TypeCheckMode::None)
      {
        log::info!(
          "Disabling Deno type checking for Next.js compile; Next handles app compilation itself"
        );
        flags.type_check_mode = TypeCheckMode::None;
      }
      // Write a temporary entrypoint file with a random suffix so we
      // never overwrite an existing project file.
      let entrypoint_path = dir.join(format!(
        ".deno_compile_entry_{:08x}.ts",
        rand::thread_rng().r#gen::<u32>()
      ));
      std::fs::write(&entrypoint_path, detection.entrypoint_code)?;
      compile_flags.source_file = entrypoint_path.display().to_string();
      if compile_flags.output.is_none()
        && let Some(dir_name) = dir.file_name()
      {
        compile_flags.output = Some(dir_name.to_string_lossy().into_owned());
      }
      // Add framework build output to includes, resolved relative to the
      // detected app directory so `deno compile ./myapp` picks up
      // `./myapp/.next` rather than `./.next`.
      for inc in detection.include_paths {
        let resolved = dir.join(&inc).display().to_string();
        if !compile_flags.include.contains(&resolved) {
          compile_flags.include.push(resolved);
        }
      }
      Some(entrypoint_path)
    } else {
      bail!(
        "Could not detect a supported framework in '{}'.\n\
         Supported frameworks: Next.js, Astro, Fresh, Remix, SvelteKit, Nuxt, SolidStart, TanStack Start, Vite SSR\n\
         Provide an explicit entrypoint instead of a directory.",
        dir.display()
      );
    }
  } else {
    None
  };

  // Keep flags.subcommand in sync so resolve_main_module sees the
  // rewritten source_file instead of the original directory path.
  flags.subcommand = DenoSubcommand::Compile(compile_flags.clone());

  // Clean up temp entrypoint on exit.
  struct CleanupGuard(Option<PathBuf>);
  impl Drop for CleanupGuard {
    fn drop(&mut self) {
      if let Some(ref path) = self.0 {
        let _ = std::fs::remove_file(path);
      }
    }
  }
  let _cleanup = CleanupGuard(_framework_entrypoint_file);

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
    compile_eszip(flags, compile_flags).boxed_local().await?;
  } else {
    compile_binary(flags, compile_flags, false)
      .boxed_local()
      .await?;
  }

  Ok(())
}

pub async fn compile_binary(
  flags: Arc<Flags>,
  compile_flags: CompileFlags,
  is_desktop: bool,
) -> Result<PathBuf, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let module_graph_creator = factory.module_graph_creator().await?;
  let binary_writer = factory.create_compile_binary_writer(is_desktop).await?;
  let entrypoint = cli_options.resolve_main_module()?;
  let bin_name_resolver = factory.bin_name_resolver()?;
  let output_path = resolve_compile_executable_output_path(
    &bin_name_resolver,
    &compile_flags,
    cli_options.initial_cwd(),
    is_desktop,
  )
  .await?;
  let compile_config = cli_options.start_dir.to_compile_config()?;
  let mut effective_include = compile_config.include.clone();
  for inc in &compile_flags.include {
    if !effective_include.contains(inc) {
      effective_include.push(inc.clone());
    }
  }
  let mut effective_exclude = compile_config.exclude.clone();
  for exc in &compile_flags.exclude {
    if !effective_exclude.contains(exc) {
      effective_exclude.push(exc.clone());
    }
  }
  let (module_roots, include_paths) = get_module_roots_and_include_paths(
    entrypoint,
    &effective_include,
    &effective_exclude,
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

  // Clean up stale temp files from previous interrupted compilations.
  if let Some(parent) = output_path.parent() {
    if let Some(stem) = output_path.file_name() {
      let prefix = format!("{}.tmp-", stem.to_string_lossy());
      if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
          if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
          }
        }
      }
    }
  }

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
      exclude_paths: effective_exclude
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

  Ok(output_path)
}

/// Convert a PNG image to macOS .icns format using `sips` and `iconutil`.
pub fn convert_png_to_icns(
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
    bail!(
      "Failed to convert PNG to ICNS. Provide an .icns file directly or ensure iconutil is available."
    );
  }

  Ok(())
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), AnyError> {
  std::fs::create_dir_all(dst)?;
  for entry in std::fs::read_dir(src)
    .with_context(|| format!("Reading directory '{}'", src.display()))?
  {
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
    false,
  )
  .await?;
  output_path.set_extension("eszip");

  let maybe_import_map_specifier =
    cli_options.resolve_specified_import_map_specifier()?;
  let compile_config = cli_options.start_dir.to_compile_config()?;
  let mut effective_include = compile_config.include.clone();
  for inc in &compile_flags.include {
    if !effective_include.contains(inc) {
      effective_include.push(inc.clone());
    }
  }
  let mut effective_exclude = compile_config.exclude.clone();
  for exc in &compile_flags.exclude {
    if !effective_exclude.contains(exc) {
      effective_exclude.push(exc.clone());
    }
  }
  let (module_roots, _include_paths) = get_module_roots_and_include_paths(
    entrypoint,
    &effective_include,
    &effective_exclude,
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
  include: &[String],
  exclude: &[String],
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
  let exclude_set = exclude
    .iter()
    .map(|path| initial_cwd.join(path))
    .collect::<HashSet<_>>();
  module_roots.push(entrypoint.clone());
  for side_module in include {
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
  is_desktop: bool,
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
    if is_desktop {
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
      },
      &resolve_cwd(None).unwrap(),
      false,
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
      },
      &resolve_cwd(None).unwrap(),
      false,
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
