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
use deno_graph::ModuleGraph;
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
use crate::graph_util::ModuleGraphCreator;
use crate::standalone::binary::WriteBinOptions;
use crate::standalone::binary::is_standalone_binary;
use crate::util::file_watcher;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::temp::create_temp_node_modules_dir;

pub async fn compile(
  flags: Flags,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  if let Some(watch_flags) = &compile_flags.watch {
    let no_clear_screen = watch_flags.no_clear_screen;
    file_watcher::watch_func(
      Arc::new(flags),
      file_watcher::PrintConfig::new("Compile", !no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let compile_flags = compile_flags.clone();
        watcher_communicator.show_path_changed(changed_paths);
        Ok(async move {
          compile_inner(
            Arc::unwrap_or_clone(flags),
            compile_flags,
            Some(watcher_communicator),
          )
          .await
        })
      },
    )
    .await
  } else {
    compile_inner(flags, compile_flags, None).await
  }
}

async fn compile_inner(
  mut flags: Flags,
  mut compile_flags: CompileFlags,
  watcher_communicator: Option<Arc<WatcherCommunicator>>,
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
      // These frameworks emit a pre-built/bundled server entrypoint that is
      // not meant to be type checked by Deno (it references Node types that
      // aren't resolvable from the build output); the framework handles its
      // own compilation.
      if matches!(detection.name, "Next.js" | "SvelteKit")
        && !matches!(flags.type_check_mode, TypeCheckMode::None)
      {
        log::info!(
          "Disabling Deno type checking for {} compile; the framework handles app compilation itself",
          detection.name
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
         Supported frameworks: Next.js, Astro, Fresh, Remix, SvelteKit, Nuxt, SolidStart, TanStack Start, Vite\n\
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

  // Clean up temp entrypoints on exit (framework-detected and/or bundled).
  struct CleanupGuard(Vec<PathBuf>);
  impl Drop for CleanupGuard {
    fn drop(&mut self) {
      for path in &self.0 {
        let _ = std::fs::remove_file(path);
      }
    }
  }
  // Register the framework entrypoint for cleanup up front so it's removed
  // even if a later step (e.g. bundling) fails and unwinds via `?`.
  let _framework_cleanup =
    _framework_entrypoint_file.map(|p| CleanupGuard(vec![p]));

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

  let _bundle_cleanup = if compile_flags.bundle {
    log::warn!(
      "{} deno compile --bundle is experimental and may change.",
      colors::yellow("Warning")
    );
    let original_source_file = compile_flags.source_file.clone();
    // Auto-include the closest `package.json` to the entrypoint, if any.
    // Lots of packages read their own `package.json` for version info
    // (pi's `getPackageJsonPath()` walks up from `import.meta.url`),
    // and after bundling the bundle's URL doesn't sit next to one. We
    // ship it alongside the bundle so the walk-up succeeds without the
    // user having to thread `--include` themselves.
    let initial_cwd_for_pkg = flags.initial_cwd.clone().unwrap_or_else(|| {
      crate::util::env::resolve_cwd(None).unwrap().to_path_buf()
    });
    if let Some(pkg_json_path) =
      closest_package_json(&initial_cwd_for_pkg, &compile_flags.source_file)
    {
      let display = pkg_json_path.display().to_string();
      if !compile_flags.include.contains(&display) {
        compile_flags.include.push(display);
      }
    }
    let BundleForCompileResult {
      path: bundle_path,
      needs_npm_embed,
      referenced_abs_paths,
      extra_cleanup,
    } = run_bundle_for_compile(&flags, &compile_flags)
      .boxed_local()
      .await?;
    flags.internal.compile_bundle_embed_node_modules = needs_npm_embed;
    flags.internal.compile_bundle_original_source_file =
      Some(original_source_file);
    // Referenced files that live inside a `node_modules` tree are npm
    // packages, embedded by the binary writer's npm path. The rest are local
    // project files the bundle externalized (e.g. a sibling `.cjs` imported
    // from ESM, which the CJS-from-ESM wrapper turns into a runtime
    // require()). Those aren't covered by the npm embed, so add them to the
    // include set to ship them in the VFS at their real path — that's where
    // `__internalResolveBundlePath` looks for them at runtime.
    for path in &referenced_abs_paths {
      let in_node_modules =
        path.components().any(|c| c.as_os_str() == "node_modules");
      if !in_node_modules {
        let included = path.display().to_string();
        if !compile_flags.include.contains(&included) {
          compile_flags.include.push(included);
        }
      }
    }
    flags.internal.compile_bundle_referenced_paths = referenced_abs_paths;
    compile_flags.source_file = bundle_path.to_string_lossy().into_owned();
    // Make sure any worker bundles travel along in the VFS so the runtime
    // `new Worker(new URL(..., import.meta.url))` lookup hits them.
    for worker_path in &extra_cleanup {
      compile_flags
        .include
        .push(worker_path.display().to_string());
    }
    flags.subcommand = DenoSubcommand::Compile(compile_flags.clone());
    let mut cleanup = vec![bundle_path];
    cleanup.extend(extra_cleanup);
    Some(CleanupGuard(cleanup))
  } else {
    None
  };

  let flags = Arc::new(flags);
  // boxed_local() is to avoid large futures
  if compile_flags.eszip {
    compile_eszip(flags, compile_flags, watcher_communicator)
      .boxed_local()
      .await?;
  } else {
    compile_binary(flags, compile_flags, false, watcher_communicator)
      .boxed_local()
      .await?;
  }

  Ok(())
}

struct BundleForCompileResult {
  path: PathBuf,
  /// True when esbuild's CJS-from-ESM wrapper appears in the bundle (in the
  /// main entry or any worker), which means runtime require()s against npm
  /// package paths will happen. The standalone binary writer reads this to
  /// decide whether to embed the npm tree.
  needs_npm_embed: bool,
  /// Absolute paths the bundle path-rewriter resolved. Used downstream to
  /// scope the npm-tree embed to just the packages those paths live in.
  referenced_abs_paths: Vec<PathBuf>,
  /// Worker bundle files produced alongside the main one; they live next to
  /// the main bundle and must be cleaned up too.
  extra_cleanup: Vec<PathBuf>,
}

async fn run_bundle_for_compile(
  flags: &Flags,
  compile_flags: &CompileFlags,
) -> Result<BundleForCompileResult, AnyError> {
  let bundle_flags = Arc::new(flags.clone());
  let initial_cwd = flags.initial_cwd.clone().unwrap_or_else(|| {
    crate::util::env::resolve_cwd(None).unwrap().to_path_buf()
  });

  let main_bytes = bundle_one_for_compile(
    bundle_flags.clone(),
    compile_flags.source_file.clone(),
    compile_flags.minify,
  )
  .await?;
  let main_rewrite = rewrite_absolute_bundle_paths(&main_bytes, &initial_cwd)?;
  let mut needs_npm_embed = main_rewrite
    .referenced_abs_paths
    .iter()
    .any(|path| path_is_in_node_modules(path));
  let mut all_referenced_paths: Vec<PathBuf> =
    main_rewrite.referenced_abs_paths.clone();

  // Scan the main bundle for `new URL("X.{ts,js,…}", import.meta.url)`
  // patterns. Each resolvable target is a potential worker entrypoint —
  // including ones the user code stashes in a variable before passing
  // to `new Worker(...)`, which the previous inline-only regex missed.
  // For each unique target we bundle it separately, write it next to
  // the main bundle, and rewrite the URL string in the main bundle to
  // point at the worker bundle's file name.
  let main_src = std::str::from_utf8(&main_rewrite.bytes)
    .context("Bundle output is not valid UTF-8")?;
  let worker_urls = discover_worker_urls(main_src, &initial_cwd);

  let mut url_replacements: Vec<(String, String)> = Vec::new();
  let mut extra_cleanup: Vec<PathBuf> = Vec::new();
  for (worker_url, worker_abs) in &worker_urls {
    let worker_bytes = bundle_one_for_compile(
      bundle_flags.clone(),
      worker_abs.display().to_string(),
      compile_flags.minify,
    )
    .await?;
    let worker_rewrite =
      rewrite_absolute_bundle_paths(&worker_bytes, &initial_cwd)?;
    needs_npm_embed |= worker_rewrite
      .referenced_abs_paths
      .iter()
      .any(|path| path_is_in_node_modules(path));
    all_referenced_paths.extend(worker_rewrite.referenced_abs_paths.clone());

    let worker_path = initial_cwd.join(format!(
      ".deno_compile_worker_{:08x}.mjs",
      rand::thread_rng().r#gen::<u32>()
    ));
    std::fs::write(&worker_path, &worker_rewrite.bytes).with_context(|| {
      format!(
        "Writing bundled worker entrypoint to '{}'",
        worker_path.display()
      )
    })?;
    let worker_file_name = worker_path
      .file_name()
      .unwrap()
      .to_string_lossy()
      .into_owned();
    url_replacements
      .push((worker_url.clone(), format!("./{worker_file_name}")));
    extra_cleanup.push(worker_path);
  }

  // Apply URL replacements to the main bundle source.
  let final_main_src = if url_replacements.is_empty() {
    main_src.to_string()
  } else {
    rewrite_worker_urls(main_src, &url_replacements)
  };

  let bundle_path = initial_cwd.join(format!(
    ".deno_compile_bundle_{:08x}.mjs",
    rand::thread_rng().r#gen::<u32>()
  ));
  std::fs::write(&bundle_path, final_main_src.as_bytes()).with_context(
    || format!("Writing bundled entrypoint to '{}'", bundle_path.display()),
  )?;

  Ok(BundleForCompileResult {
    path: bundle_path,
    needs_npm_embed,
    referenced_abs_paths: all_referenced_paths,
    extra_cleanup,
  })
}

async fn bundle_one_for_compile(
  flags: Arc<Flags>,
  entrypoint: String,
  minify: bool,
) -> Result<Vec<u8>, AnyError> {
  // Always leave `.node` files external. esbuild has no loader for them
  // and would error if it tried to inline a native binary; with this
  // pattern the require() calls are emitted verbatim and resolved at
  // runtime against the embedded VFS by the native addon loader.
  let external = vec!["*.node".to_string()];
  super::bundle::bundle_for_compile(flags, entrypoint, external, minify)
    .boxed_local()
    .await
}

/// Find every `new URL("X.{ts,js,…}", import.meta.url)` in the bundle whose
/// `X` we can resolve to a source file on disk. Each match is a potential
/// worker entrypoint: even when the URL is stashed in a variable and only
/// later passed to `new Worker(...)` we still want to bundle it.
///
/// Resolution tries the URL as a relative path from `initial_cwd` first
/// (covers the common case where source and bundle share a directory),
/// then falls back to a basename search across the workspace — pi's
/// `dist/utils/image-resize.js` does
/// `new URL("./image-resize-worker.js", import.meta.url)` and the bundle
/// lives at `dist/.deno_compile_bundle_*.mjs`, so basename matching is
/// what locks it onto `dist/utils/image-resize-worker.js`.
///
/// Returns `(original_url_string, resolved_source_path)` pairs in source
/// order, deduped on the URL string.
fn discover_worker_urls(
  bundle_src: &str,
  initial_cwd: &Path,
) -> Vec<(String, PathBuf)> {
  // Match `new URL(<first-arg>, import.meta.url)` with `<first-arg>`
  // captured non-greedily so we handle non-literal forms like the
  // ternary pi uses:
  //   new URL(isTs ? "./worker.ts" : "./worker.js", import.meta.url)
  let call_pattern = lazy_regex::regex!(
    r#"new\s+URL\s*\(\s*(.+?)\s*,\s*import\.meta\.url\s*\)"#
  );
  let url_string_pattern =
    lazy_regex::regex!(r#""([^"\n]+\.(?:ts|tsx|js|jsx|mjs|cjs))""#);
  let mut seen = std::collections::HashSet::new();
  let mut out = Vec::new();
  for call_caps in call_pattern.captures_iter(bundle_src) {
    let first_arg = call_caps.get(1).unwrap().as_str();
    for url_caps in url_string_pattern.captures_iter(first_arg) {
      let url = url_caps.get(1).unwrap().as_str().to_string();
      if !seen.insert(url.clone()) {
        continue;
      }
      if let Some(path) = resolve_worker_url_target(&url, initial_cwd) {
        out.push((url, path));
      }
    }
  }
  out
}

fn resolve_worker_url_target(url: &str, initial_cwd: &Path) -> Option<PathBuf> {
  // Try as a literal relative path from the bundle's directory first.
  let candidate = if Path::new(url).is_absolute() {
    PathBuf::from(url)
  } else {
    initial_cwd.join(url)
  };
  if candidate.is_file() {
    return Some(candidate);
  }
  // Fall back to searching by basename across the workspace. This handles
  // bundles whose runtime location doesn't match the original source's
  // location: the URL string is preserved by esbuild but
  // `import.meta.url` now points at the bundle's directory.
  let basename = Path::new(url).file_name()?;
  find_file_by_name(initial_cwd, basename)
}

fn find_file_by_name(root: &Path, name: &std::ffi::OsStr) -> Option<PathBuf> {
  let mut pending = std::collections::VecDeque::from([root.to_path_buf()]);
  while let Some(dir) = pending.pop_front() {
    let Ok(entries) = std::fs::read_dir(&dir) else {
      continue;
    };
    for entry in entries.flatten() {
      let path = entry.path();
      let file_name = path.file_name();
      if path.is_dir() {
        if matches!(
          file_name,
          Some(n) if n == std::ffi::OsStr::new("node_modules")
            || n == std::ffi::OsStr::new(".git")
            || n == std::ffi::OsStr::new("target")
        ) {
          continue;
        }
        pending.push_back(path);
      } else if file_name == Some(name) {
        return Some(path);
      }
    }
  }
  None
}

/// Find the closest `package.json` above the entrypoint. Walks up from
/// the entrypoint's directory toward `initial_cwd` and returns the first
/// `package.json` it sees. Used to auto-include the file in the VFS so
/// `getPackageDir`-style walks at runtime succeed without the user
/// having to add a `--include` flag.
fn closest_package_json(
  initial_cwd: &Path,
  source_file: &str,
) -> Option<PathBuf> {
  let source_path = if Path::new(source_file).is_absolute() {
    PathBuf::from(source_file)
  } else {
    initial_cwd.join(source_file)
  };
  let mut dir = source_path.parent()?.to_path_buf();
  loop {
    let candidate = dir.join("package.json");
    if candidate.is_file() {
      return Some(candidate);
    }
    if !dir.pop() {
      return None;
    }
  }
}

fn path_is_in_node_modules(path: &Path) -> bool {
  path.components().any(|c| c.as_os_str() == "node_modules")
}

fn rewrite_worker_urls(
  bundle_src: &str,
  replacements: &[(String, String)],
) -> String {
  // Rewrite happens inside `new URL(<arg>, import.meta.url)` only, so
  // user code that happens to contain a string matching a worker path
  // somewhere else (a log message, a regex, etc.) is left alone. Within
  // each call's `<arg>` we do a literal string-substring substitution
  // so ternaries like
  //   new URL(isTs ? "./worker.ts" : "./worker.js", import.meta.url)
  // get all of their string-literal branches rewritten.
  let pattern = lazy_regex::regex!(
    r#"(new\s+URL\s*\(\s*)(.+?)(\s*,\s*import\.meta\.url\s*\))"#
  );
  pattern
    .replace_all(bundle_src, |caps: &regex::Captures<'_>| {
      let prefix = &caps[1];
      let mut arg = caps[2].to_string();
      let suffix = &caps[3];
      for (orig, replacement) in replacements {
        let needle = format!("\"{orig}\"");
        let with = format!("\"{replacement}\"");
        arg = arg.replace(&needle, &with);
      }
      format!("{prefix}{arg}{suffix}")
    })
    .into_owned()
}

struct RewriteResult {
  bytes: Vec<u8>,
  #[allow(
    dead_code,
    reason = "read by unit tests on platforms with rewriting"
  )]
  rewrote_paths: bool,
  /// Absolute paths the rewriter touched. These point at the build-machine
  /// locations of files the bundled output expects to require at runtime
  /// — typically deep inside the npm cache. The binary writer uses this
  /// set to decide which npm packages to embed in the VFS.
  referenced_abs_paths: Vec<PathBuf>,
}

fn rewrite_absolute_bundle_paths(
  bundle_bytes: &[u8],
  bundle_dir: &Path,
) -> Result<RewriteResult, AnyError> {
  let src = std::str::from_utf8(bundle_bytes)
    .context("Bundle output is not valid UTF-8")?;
  // Only rewrite paths that actually exist on disk at build time — these are
  // the ones the bundler emitted referring to files it expects to be
  // reachable through the VFS at runtime.
  Ok(rewrite_absolute_bundle_paths_inner(src, bundle_dir, |p| {
    p.exists()
  }))
}

/// Core of [`rewrite_absolute_bundle_paths`], parameterized over the
/// build-time existence check so it can be unit-tested with synthetic paths.
fn rewrite_absolute_bundle_paths_inner(
  src: &str,
  bundle_dir: &Path,
  path_exists: impl Fn(&Path) -> bool,
) -> RewriteResult {
  // Match string literals that look like an absolute path to a JS/JSON source
  // file: either POSIX (`/a/b.js`) or a Windows drive-letter path
  // (`C:\a\b.js` / `C:/a/b.js`). Because the bundle is JS source, a Windows
  // path's separators arrive JS-escaped as `\\`, which the body matches as
  // ordinary (non-`"`) characters. Conservative on extension on purpose — we
  // don't want to rewrite arbitrary user-provided strings.
  //
  // Load-bearing assumption: every absolute-path literal we match is an
  // external `require(...)` argument, i.e. a *value* position where wrapping
  // it in `__internalResolveBundlePath(...)` stays valid JS. This holds
  // because esbuild emits *relative* keys (no leading `/`) for the inlined
  // `__commonJS` module map, so the only absolute literals left in the output
  // are at external require call sites. If esbuild ever emitted an absolute
  // `__commonJS` key (e.g. via a different `absWorkingDir`/outbase), this
  // would rewrite it into `{ __internalResolveBundlePath("...")(...) {...} }`,
  // a syntax error — the spec tests under `tests/specs/compile/bundle` would
  // catch that. A genuine user string literal pointing at an existing file on
  // disk would also be rewritten, but the build-time existence check below
  // keeps that to paths that really resolve through the VFS.
  let pattern = lazy_regex::regex!(
    r#""((?:[A-Za-z]:)?[\\/][^"\n]+\.(?:js|cjs|mjs|json))""#
  );

  let mut any_rewrite = false;
  let mut referenced_abs_paths: Vec<PathBuf> = Vec::new();
  let rewritten = pattern.replace_all(src, |caps: &regex::Captures<'_>| {
    // Collapse JS-escaped backslashes (`C:\\a\\b.js`) back to real
    // separators before touching the filesystem.
    let abs = caps.get(1).unwrap().as_str().replace("\\\\", "\\");
    let path = Path::new(&abs);
    if !path_exists(path) {
      return caps[0].to_string();
    }
    // Rewrite to a path relative to the bundle file. This is correct only
    // because the VFS embeds `node_modules` at the same cwd-relative offset
    // from the bundle that `diff_paths` computes here (bundle_dir =
    // initial_cwd at build time, resolved at runtime against
    // `import.meta.url`). See `fill_npm_vfs` in cli/standalone/binary.rs,
    // which preserves that cwd-relative layout when populating the VFS.
    let Some(rel) = pathdiff::diff_paths(path, bundle_dir) else {
      return caps[0].to_string();
    };
    any_rewrite = true;
    referenced_abs_paths.push(path.to_path_buf());
    let rel_str: String = rel.to_string_lossy().replace('\\', "/");
    format!("__internalResolveBundlePath({:?})", rel_str.as_str())
  });

  if !any_rewrite {
    return RewriteResult {
      bytes: src.as_bytes().to_vec(),
      rewrote_paths: false,
      referenced_abs_paths,
    };
  }

  let prefix = r#"// Injected by deno compile --bundle: resolve absolute paths emitted by
// esbuild's CJS-from-ESM wrapper against the bundle file's runtime
// location instead of the build-time absolute path.
import { fileURLToPath as __internalFileURLToPath } from "node:url";
import * as __internalPath from "node:path";
const __internalBundleDir = __internalPath.dirname(__internalFileURLToPath(import.meta.url));
function __internalResolveBundlePath(rel) {
  return __internalPath.join(__internalBundleDir, rel);
}
"#;
  RewriteResult {
    bytes: format!("{prefix}{rewritten}").into_bytes(),
    rewrote_paths: true,
    referenced_abs_paths,
  }
}

pub async fn compile_binary(
  flags: Arc<Flags>,
  compile_flags: CompileFlags,
  is_desktop: bool,
  watcher_communicator: Option<Arc<WatcherCommunicator>>,
) -> Result<PathBuf, AnyError> {
  let factory = if let Some(watcher_communicator) = watcher_communicator.clone()
  {
    CliFactory::from_flags_for_watcher(flags, watcher_communicator)
  } else {
    CliFactory::from_flags(flags)
  };
  let cli_options = factory.cli_options()?;
  let module_graph_creator = factory.module_graph_creator().await?;
  let binary_writer = factory.create_compile_binary_writer(is_desktop).await?;
  let entrypoint = cli_options.resolve_main_module()?;
  let bin_name_resolver = factory.bin_name_resolver()?;
  let output_path = resolve_compile_executable_output_path(
    &bin_name_resolver,
    &compile_flags,
    cli_options
      .compile_bundle_original_source_file()
      .unwrap_or(&compile_flags.source_file),
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
  let roots = get_module_roots_and_include_paths(
    entrypoint,
    &effective_include,
    &effective_exclude,
    cli_options,
  )?;
  watch_compile_paths(
    watcher_communicator.as_ref(),
    &roots,
    &compile_flags,
    cli_options.initial_cwd(),
  );

  let graph =
    build_compile_graph(module_graph_creator, cli_options, &roots).await?;

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
  if let Some(parent) = output_path.parent()
    && let Some(stem) = output_path.file_name()
  {
    let prefix = format!("{}.tmp-", stem.to_string_lossy());
    if let Ok(entries) = std::fs::read_dir(parent) {
      for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
          let _ = std::fs::remove_file(entry.path());
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
      include_paths: &roots.include_paths,
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
  watcher_communicator: Option<Arc<WatcherCommunicator>>,
) -> Result<(), AnyError> {
  let factory = if let Some(watcher_communicator) = watcher_communicator.clone()
  {
    CliFactory::from_flags_for_watcher(flags, watcher_communicator)
  } else {
    CliFactory::from_flags(flags)
  };
  let cli_options = factory.cli_options()?;
  let module_graph_creator = factory.module_graph_creator().await?;
  let parsed_source_cache = factory.parsed_source_cache()?;
  let compiler_options_resolver = factory.compiler_options_resolver()?;
  let bin_name_resolver = factory.bin_name_resolver()?;
  let entrypoint = cli_options.resolve_main_module()?;
  let mut output_path = resolve_compile_executable_output_path(
    &bin_name_resolver,
    &compile_flags,
    cli_options
      .compile_bundle_original_source_file()
      .unwrap_or(&compile_flags.source_file),
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
  let roots = get_module_roots_and_include_paths(
    entrypoint,
    &effective_include,
    &effective_exclude,
    cli_options,
  )?;
  watch_compile_paths(
    watcher_communicator.as_ref(),
    &roots,
    &compile_flags,
    cli_options.initial_cwd(),
  );

  let graph =
    build_compile_graph(module_graph_creator, cli_options, &roots).await?;

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

struct CompileModuleRoots {
  /// Strict graph roots (entrypoint, `--preload` and `--require` modules)
  /// whose graph resolution errors should fail compilation.
  strict: Vec<ModuleSpecifier>,
  /// JS-like files brought in via `--include`; they are embedded and
  /// transpiled but treated as best-effort assets, so their graph resolution
  /// errors must not fail compilation (see #27505).
  include: Vec<ModuleSpecifier>,
  /// Raw files/directories embedded into the VFS.
  include_paths: Vec<ModuleSpecifier>,
}

/// Builds the module graph stored in the compiled binary.
///
/// Only the strict roots are validated/type checked; `--include` modules are
/// best-effort assets whose unresolved imports are embedded as-is rather than
/// surfaced as errors (#27505).
async fn build_compile_graph(
  module_graph_creator: &ModuleGraphCreator,
  cli_options: &CliOptions,
  roots: &CompileModuleRoots,
) -> Result<ModuleGraph, AnyError> {
  let checked_graph = module_graph_creator
    .create_graph_and_maybe_check(roots.strict.clone())
    .await?;

  if roots.include.is_empty() && !cli_options.type_check_mode().is_true() {
    // Fast path: no includes and no type checking, so the validated graph is
    // exactly what we want to store.
    Ok(Arc::try_unwrap(checked_graph).unwrap())
  } else {
    // Build a code-only graph that also includes the `--include` module roots.
    // `create_graph` does not validate, so unresolved imports inside included
    // assets are embedded as-is rather than surfaced as errors. We also use
    // this path after type checking so type information isn't stored in the
    // binary.
    let mut all_roots = roots.strict.clone();
    all_roots.extend(roots.include.iter().cloned());
    module_graph_creator
      .create_graph(GraphKind::CodeOnly, all_roots, NpmCachingStrategy::Eager)
      .await
  }
}

fn watch_compile_paths(
  watcher_communicator: Option<&Arc<WatcherCommunicator>>,
  roots: &CompileModuleRoots,
  compile_flags: &CompileFlags,
  initial_cwd: &Path,
) {
  let Some(watcher_communicator) = watcher_communicator else {
    return;
  };

  let paths = compile_watch_paths(roots, compile_flags, initial_cwd);

  if !paths.is_empty() {
    let _ = watcher_communicator.watch_paths(paths);
  }
}

fn compile_watch_paths(
  roots: &CompileModuleRoots,
  compile_flags: &CompileFlags,
  initial_cwd: &Path,
) -> Vec<PathBuf> {
  let mut paths = roots
    .include_paths
    .iter()
    .filter_map(|specifier| url_to_file_path(specifier).ok())
    .collect::<Vec<_>>();

  if let Some(icon) = compile_flags.icon.as_ref()
    && let Ok(specifier) = resolve_url_or_path(icon, initial_cwd)
    && let Ok(path) = url_to_file_path(&specifier)
  {
    paths.push(path);
  }

  paths
}

fn get_module_roots_and_include_paths(
  entrypoint: &ModuleSpecifier,
  include: &[String],
  exclude: &[String],
  cli_options: &Arc<CliOptions>,
) -> Result<CompileModuleRoots, AnyError> {
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
  let mut include_module_roots = Vec::new();
  let mut include_paths = Vec::new();
  let exclude_set = exclude
    .iter()
    .map(|path| initial_cwd.join(path))
    .collect::<HashSet<_>>();
  module_roots.push(entrypoint.clone());
  for side_module in include {
    let url = resolve_url_or_path(side_module, initial_cwd)?;
    if is_module_graph_module(&url) {
      include_module_roots.push(url.clone());
    } else {
      analyze_path(&url, &exclude_set, &mut searched_paths, |file_path| {
        let media_type = MediaType::from_path(file_path);
        if is_module_graph_media_type(media_type)
          && let Ok(file_url) = url_from_file_path(file_path)
        {
          include_module_roots.push(file_url);
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

  Ok(CompileModuleRoots {
    strict: module_roots,
    include: include_module_roots,
    include_paths,
  })
}

async fn resolve_compile_executable_output_path(
  bin_name_resolver: &BinNameResolver<'_>,
  compile_flags: &CompileFlags,
  source_file_for_inference: &str,
  current_dir: &Path,
  is_desktop: bool,
) -> Result<PathBuf, AnyError> {
  let module_specifier =
    resolve_url_or_path(source_file_for_inference, current_dir)?;

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
  use std::collections::HashMap;

  use deno_npm::registry::TestNpmRegistryApi;
  use deno_npm::resolution::NpmVersionResolver;

  pub use super::*;
  use crate::http_util::HttpClientProvider;
  use crate::util::env::resolve_cwd;

  #[test]
  fn compile_watch_paths_include_includes_and_icon() {
    let initial_cwd = resolve_cwd(None).unwrap();
    let included_path = initial_cwd.join("data.txt");
    let roots = CompileModuleRoots {
      strict: vec![],
      include: vec![],
      include_paths: vec![url_from_file_path(&included_path).unwrap()],
    };
    let paths = compile_watch_paths(
      &roots,
      &CompileFlags {
        source_file: "mod.ts".to_string(),
        output: None,
        args: Vec::new(),
        target: None,
        watch: None,
        no_terminal: false,
        icon: Some("favicon.ico".to_string()),
        include: Default::default(),
        exclude: Default::default(),
        eszip: false,
        self_extracting: false,
        bundle: false,
        app_name: None,
        minify: false,
        exclude_unused_npm: false,
      },
      &initial_cwd,
    );
    assert_eq!(paths, vec![included_path, initial_cwd.join("favicon.ico")]);
  }

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
        watch: None,
        no_terminal: false,
        icon: None,
        include: Default::default(),
        exclude: Default::default(),
        eszip: true,
        self_extracting: false,
        bundle: false,
        app_name: None,
        minify: false,
        exclude_unused_npm: false,
      },
      "mod.ts",
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
        watch: None,
        include: Default::default(),
        exclude: Default::default(),
        icon: None,
        no_terminal: false,
        eszip: true,
        self_extracting: false,
        bundle: false,
        app_name: None,
        minify: false,
        exclude_unused_npm: false,
      },
      "mod.ts",
      &resolve_cwd(None).unwrap(),
      false,
    )
    .await
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "file.exe");
  }

  #[tokio::test]
  async fn resolve_compile_output_path_bundle_uses_original_npm_source() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let http_client = HttpClientProvider::new(None, None);
    let npm_api = TestNpmRegistryApi::default();
    npm_api.with_version_info(("@google/gemini-cli", "1.0.0"), |info| {
      info.bin = Some(deno_npm::registry::NpmPackageVersionBinEntry::Map(
        HashMap::from([("gemini".to_string(), "./bin.js".to_string())]),
      ))
    });
    let npm_version_resolver = NpmVersionResolver::default();
    let bin_name_resolver =
      BinNameResolver::new(&http_client, &npm_api, &npm_version_resolver);
    let path = resolve_compile_executable_output_path(
      &bin_name_resolver,
      &CompileFlags {
        source_file: ".deno_compile_bundle_12345678.mjs".to_string(),
        output: None,
        args: Vec::new(),
        target: Some("x86_64-unknown-linux-gnu".to_string()),
        watch: None,
        include: Default::default(),
        exclude: Default::default(),
        icon: None,
        no_terminal: false,
        eszip: false,
        self_extracting: false,
        bundle: true,
        minify: false,
        exclude_unused_npm: false,
      },
      "npm:@google/gemini-cli",
      &resolve_cwd(None).unwrap(),
      false,
    )
    .await
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "gemini");
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

  #[test]
  fn test_rewrite_absolute_bundle_paths_native() {
    // Use the platform-native absolute path layout so this exercises the
    // real shape esbuild emits on each OS. On Windows the require() string in
    // the bundle is a drive-letter path with JS-escaped backslashes
    // (`C:\\proj\\dist\\pkg\\index.js`), which the previous Unix-only regex
    // never matched — leaving the require pointed at a non-existent
    // build-time path at runtime.
    let bundle_dir = if cfg!(windows) {
      PathBuf::from("C:\\proj\\dist")
    } else {
      PathBuf::from("/proj/dist")
    };
    let abs = bundle_dir.join("pkg").join("index.js");
    // Escape backslashes the way they appear inside a JS string literal.
    let abs_in_js = abs.to_string_lossy().replace('\\', "\\\\");
    let src = format!("var m = require(\"{abs_in_js}\");\n");

    let result =
      rewrite_absolute_bundle_paths_inner(&src, &bundle_dir, |_| true);

    assert!(result.rewrote_paths);
    let out = String::from_utf8(result.bytes).unwrap();
    assert!(
      out.contains(r#"__internalResolveBundlePath("pkg/index.js")"#),
      "unexpected output: {out}"
    );
  }

  #[test]
  fn test_rewrite_absolute_bundle_paths_skips_missing() {
    let bundle_dir = PathBuf::from("/proj/dist");
    let src = "var m = require(\"/does/not/exist.js\");\n";
    let result =
      rewrite_absolute_bundle_paths_inner(src, &bundle_dir, |_| false);
    assert!(!result.rewrote_paths);
    assert_eq!(result.bytes.as_slice(), src.as_bytes());
  }

  #[test]
  fn test_rewrite_absolute_bundle_paths_skips_relative_key() {
    // esbuild emits *relative* keys (no leading `/`) for inlined
    // `__commonJS` modules. Those are object-literal keys, not require()
    // arguments — rewriting one into `__internalResolveBundlePath("...")`
    // would be a syntax error. The leading-separator anchor in the pattern
    // keeps them untouched even when the path exists on disk (`|_| true`).
    let bundle_dir = PathBuf::from("/proj/dist");
    let src = "var b = { \"pkg/index.js\"(exports, module) { module.exports = 1; } };\n";
    let result =
      rewrite_absolute_bundle_paths_inner(src, &bundle_dir, |_| true);
    assert!(!result.rewrote_paths);
    assert_eq!(result.bytes.as_slice(), src.as_bytes());
  }
}
