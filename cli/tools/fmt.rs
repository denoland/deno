// Copyright 2018-2026 the Deno authors. MIT license.

//! This module provides file formatting utilities using
//! [`dprint-plugin-typescript`](https://github.com/dprint/dprint-plugin-typescript).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

use std::borrow::Cow;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::io::stdin;
use std::io::stdout;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use deno_ast::ParsedSource;
use deno_config::deno_json::NewLineKind;
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::unsync::spawn_blocking;
use log::debug;
use log::info;
use log::warn;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::FmtFlags;
use crate::args::FmtOptions;
use crate::args::FmtOptionsConfig;
use crate::args::ProseWrap;
use crate::args::UnstableFmtOptions;
use crate::cache::CacheDBHash;
use crate::cache::Caches;
use crate::cache::IncrementalCache;
use crate::colors;
use crate::factory::CliFactory;
use crate::sys::CliSys;
use crate::tools::fmt_editorconfig::EditorConfigCache;
use crate::util;
use crate::util::file_watcher;
use crate::util::fs::canonicalize_path;
use crate::util::path::get_extension;

/// Format JavaScript/TypeScript files.
pub async fn format(
  flags: Arc<Flags>,
  fmt_flags: FmtFlags,
) -> Result<(), AnyError> {
  if fmt_flags.is_stdin() {
    let factory = CliFactory::from_flags(flags);
    let cli_options = factory.cli_options()?;
    let start_dir = &cli_options.start_dir;
    let fmt_config = start_dir
      .to_fmt_config(FilePatterns::new_with_base(start_dir.dir_path()))?;
    let fmt_options = FmtOptions::resolve(
      fmt_config,
      cli_options.resolve_config_unstable_fmt_options(),
      &fmt_flags,
    );
    return format_stdin(
      &fmt_flags,
      fmt_options,
      cli_options
        .ext_flag()
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("ts"),
    );
  }

  if let Some(watch_flags) = &fmt_flags.watch {
    file_watcher::watch_func(
      flags,
      file_watcher::PrintConfig::new("Fmt", !watch_flags.no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let fmt_flags = fmt_flags.clone();
        watcher_communicator.show_path_changed(changed_paths.clone());
        Ok(async move {
          let factory = CliFactory::from_flags(flags);
          let cli_options = factory.cli_options()?;
          let caches = factory.caches()?;
          let mut paths_with_options_batches =
            resolve_paths_with_options_batches(cli_options, &fmt_flags)?;

          for paths_with_options in &mut paths_with_options_batches {
            let _ = watcher_communicator.watch_paths(
              file_watcher::watch_paths_for_file_patterns(
                &paths_with_options.options.files,
              ),
            );
            let files = std::mem::take(&mut paths_with_options.paths);
            paths_with_options.paths = if let Some(paths) = &changed_paths {
              if fmt_flags.check {
                // check all files on any changed (https://github.com/denoland/deno/issues/12446)
                if files.iter().any(|path| {
                  canonicalize_path(path)
                    .map(|path| paths.contains(&path))
                    .unwrap_or(false)
                }) {
                  files
                } else {
                  [].to_vec()
                }
              } else {
                files
                  .into_iter()
                  .filter(|path| {
                    canonicalize_path(path)
                      .map(|path| paths.contains(&path))
                      .unwrap_or(false)
                  })
                  .collect::<Vec<_>>()
              }
            } else {
              files
            };
          }

          format_files(
            caches,
            cli_options,
            &fmt_flags,
            paths_with_options_batches,
          )
          .await?;

          Ok(())
        })
      },
    )
    .await?;
  } else {
    let factory = CliFactory::from_flags(flags);
    let cli_options = factory.cli_options()?;
    let caches = factory.caches()?;
    let paths_with_options_batches =
      resolve_paths_with_options_batches(cli_options, &fmt_flags)?;
    return format_files(
      caches,
      cli_options,
      &fmt_flags,
      paths_with_options_batches,
    )
    .await;
  }

  Ok(())
}

struct PathsWithOptions {
  base: PathBuf,
  paths: Vec<PathBuf>,
  options: FmtOptions,
}

fn resolve_paths_with_options_batches(
  cli_options: &CliOptions,
  fmt_flags: &FmtFlags,
) -> Result<Vec<PathsWithOptions>, AnyError> {
  maybe_show_format_confirmation(cli_options, fmt_flags)?;

  let members_fmt_options =
    cli_options.resolve_fmt_options_for_members(fmt_flags)?;
  let mut paths_with_options_batches =
    Vec::with_capacity(members_fmt_options.len());
  for (_ctx, member_fmt_options) in members_fmt_options {
    let files =
      collect_fmt_files(cli_options, member_fmt_options.files.clone());
    if !files.is_empty() {
      paths_with_options_batches.push(PathsWithOptions {
        base: member_fmt_options.files.base.clone(),
        paths: files,
        options: member_fmt_options,
      });
    }
  }
  if paths_with_options_batches.is_empty() && !fmt_flags.permit_no_files {
    return Err(anyhow!("No target files found."));
  }
  Ok(paths_with_options_batches)
}

fn maybe_show_format_confirmation(
  cli_options: &CliOptions,
  fmt_flags: &FmtFlags,
) -> Result<(), AnyError> {
  if fmt_flags.check
    || !fmt_flags.files.include.is_empty()
    || cli_options.workspace().deno_jsons().next().is_some()
    || cli_options.workspace().package_jsons().next().is_some()
  {
    return Ok(());
  }

  let confirm_result =
    util::console::confirm(util::console::ConfirmOptions {
      default: true,
      message: format!(
        "{} It looks like you're not in a workspace. Are you sure you want to format the entire '{}' directory?",
        colors::yellow("Warning"),
        cli_options.initial_cwd().display()
      ),
    })
    .unwrap_or(false);
  if confirm_result {
    Ok(())
  } else {
    bail!(
      "Did not format non-workspace directory. Run again specifying the current directory (ex. `deno fmt .`)"
    )
  }
}

async fn format_files(
  caches: &Arc<Caches>,
  cli_options: &Arc<CliOptions>,
  fmt_flags: &FmtFlags,
  paths_with_options_batches: Vec<PathsWithOptions>,
) -> Result<(), AnyError> {
  let formatter: Box<dyn Formatter> = if fmt_flags.check {
    let fail_fast = fmt_flags.fail_fast || cli_options.is_quiet();
    Box::new(CheckFormatter::new(fail_fast))
  } else {
    Box::new(RealFormatter::default())
  };
  let editorconfig_cache = Arc::new(EditorConfigCache::new());
  for paths_with_options in paths_with_options_batches {
    log::debug!(
      "Formatting {} file(s) in {}",
      paths_with_options.paths.len(),
      paths_with_options.base.display()
    );
    let fmt_options = paths_with_options.options;
    let paths = paths_with_options.paths;
    let incremental_cache = Arc::new(IncrementalCache::new(
      caches.fmt_incremental_cache_db(),
      CacheDBHash::from_hashable((&fmt_options.options, &fmt_options.unstable)),
      &paths,
    ));
    formatter
      .handle_files(
        paths,
        fmt_options.options,
        fmt_options.unstable,
        incremental_cache.clone(),
        editorconfig_cache.clone(),
        cli_options.ext_flag().clone(),
      )
      .await?;
    incremental_cache.wait_completion().await;
  }

  formatter.finish()
}

fn collect_fmt_files(
  cli_options: &CliOptions,
  files: FilePatterns,
) -> Vec<PathBuf> {
  FileCollector::new(|e| {
    is_supported_ext_fmt(e.path)
      || (e.path.extension().is_none() && cli_options.ext_flag().is_some())
  })
  .ignore_git_folder()
  .ignore_node_modules()
  .use_gitignore()
  .set_vendor_folder(cli_options.vendor_dir_path().map(ToOwned::to_owned))
  .collect_file_patterns(&CliSys::default(), &files)
}

/// Formats markdown (using <https://github.com/dprint/dprint-plugin-markdown>) and its code blocks
/// (ts/tsx, js/jsx).
fn format_markdown(
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
  unstable_options: &UnstableFmtOptions,
) -> Result<Option<String>, AnyError> {
  let markdown_config = get_resolved_markdown_config(fmt_options);
  dprint_plugin_markdown::format_text(
    file_text,
    &markdown_config,
    move |tag, text, line_width| {
      let tag = tag.to_lowercase();
      if matches!(
        tag.as_str(),
        "ts"
          | "tsx"
          | "js"
          | "jsx"
          | "cjs"
          | "cts"
          | "mjs"
          | "mts"
          | "javascript"
          | "typescript"
          | "json"
          | "jsonc"
          | "css"
          | "scss"
          | "less"
          | "html"
          | "svelte"
          | "vue"
          | "astro"
          | "vto"
          | "njk"
          | "yml"
          | "yaml"
          | "sql"
      ) {
        // It's important to tell dprint proper file extension, otherwise
        // it might parse the file twice.
        let extension = match tag.as_str() {
          "javascript" => "js",
          "typescript" => "ts",
          rest => rest,
        };

        let fake_filename =
          PathBuf::from(format!("deno_fmt_stdin.{extension}"));
        match extension {
          "json" | "jsonc" => {
            let mut json_config = get_resolved_json_config(fmt_options);
            json_config.line_width = line_width;
            dprint_plugin_json::format_text(&fake_filename, text, &json_config)
          }
          "css" | "scss" | "less" => {
            format_css(&fake_filename, text, fmt_options)
          }
          "html" | "svg" | "xml" => {
            format_html(&fake_filename, text, fmt_options, unstable_options)
          }
          "svelte" | "vue" | "astro" | "vto" | "njk" => {
            if unstable_options.component {
              format_html(&fake_filename, text, fmt_options, unstable_options)
            } else {
              Ok(None)
            }
          }
          "yml" | "yaml" => format_yaml(text, fmt_options),
          "sql" => {
            if unstable_options.sql {
              format_sql(text, fmt_options)
            } else {
              Ok(None)
            }
          }
          _ => {
            let mut codeblock_config =
              get_resolved_typescript_config(fmt_options);
            codeblock_config.line_width = line_width;
            dprint_plugin_typescript::format_text(
              dprint_plugin_typescript::FormatTextOptions {
                path: &fake_filename,
                extension: None,
                text: text.to_string(),
                config: &codeblock_config,
                external_formatter: Some(
                  &create_external_formatter_for_typescript(unstable_options),
                ),
              },
            )
          }
        }
      } else {
        Ok(None)
      }
    },
  )
}

/// Formats JSON and JSONC using the rules provided by .deno()
/// of configuration builder of <https://github.com/dprint/dprint-plugin-json>.
/// See <https://github.com/dprint/dprint-plugin-json/blob/cfa1052dbfa0b54eb3d814318034cdc514c813d7/src/configuration/builder.rs#L87> for configuration.
pub fn format_json(
  file_path: &Path,
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  let config = get_resolved_json_config(fmt_options);
  dprint_plugin_json::format_text(file_path, file_text, &config)
}

pub fn format_css(
  file_path: &Path,
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  lax_css::format_text(
    file_path,
    file_text,
    &get_resolved_lax_css_config(fmt_options),
  )
}

fn get_resolved_lax_css_config(
  options: &FmtOptionsConfig,
) -> lax_css::configuration::Configuration {
  lax_css::configuration::Configuration {
    line_width: options.line_width.unwrap_or(80),
    use_tabs: options.use_tabs.unwrap_or_default(),
    indent_width: options.indent_width.unwrap_or(2),
    new_line_kind: dprint_core::configuration::NewLineKind::LineFeed,
    ignore_node_comment_text: "deno-fmt-ignore".to_string(),
    ignore_file_comment_text: "deno-fmt-ignore-file".to_string(),
    single_line: false,
  }
}

fn format_yaml(
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  let ignore_file = file_text
    .lines()
    .take_while(|line| line.starts_with('#'))
    .any(|line| {
      line
        .strip_prefix('#')
        .unwrap()
        .trim()
        .starts_with("deno-fmt-ignore-file")
    });

  if ignore_file {
    return Ok(None);
  }

  let formatted_str =
    pretty_yaml::format_text(file_text, &get_resolved_yaml_config(fmt_options))
      .map_err(AnyError::from)?;

  Ok(if formatted_str == file_text {
    None
  } else {
    Some(formatted_str)
  })
}

pub fn format_html(
  file_path: &Path,
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
  unstable_options: &UnstableFmtOptions,
) -> Result<Option<String>, AnyError> {
  let config = get_resolved_lax_markup_config(fmt_options);
  let external = move |lang: &str, text: &str, print_width: u32| {
    format_markup_embedded(
      lang,
      text,
      print_width,
      file_path,
      fmt_options,
      unstable_options,
    )
  };
  lax_markup::format_text_with_external(
    file_path, file_text, &config, &external,
  )
}

/// Formats the contents of style and script elements in markup. The language
/// comes from the element's `lang` or `type` attribute when present, or is
/// `css`/`js` by element kind. Unknown languages are kept verbatim.
fn format_markup_embedded(
  lang: &str,
  text: &str,
  print_width: u32,
  file_path: &Path,
  fmt_options: &FmtOptionsConfig,
  unstable_options: &UnstableFmtOptions,
) -> Result<Option<String>, AnyError> {
  let lang = lang.to_ascii_lowercase();
  match lang.as_str() {
    "css" | "scss" | "less" | "text/css" => {
      let ext = match lang.as_str() {
        "scss" => "scss",
        "less" => "less",
        _ => "css",
      };
      let mut lax_css_config = get_resolved_lax_css_config(fmt_options);
      lax_css_config.line_width = print_width;
      lax_css::format_text(
        &file_path.with_extension(ext),
        text,
        &lax_css_config,
      )
    }
    "json"
    | "jsonc"
    | "application/json"
    | "application/ld+json"
    | "importmap" => {
      let mut json_config = get_resolved_json_config(fmt_options);
      json_config.line_width = print_width;
      let path = file_path.with_extension("json");
      dprint_plugin_json::format_text(&path, text, &json_config)
    }
    "js"
    | "jsx"
    | "ts"
    | "tsx"
    | "mjs"
    | "mts"
    | "javascript"
    | "typescript"
    | "module"
    | "text/javascript"
    | "application/javascript" => {
      let ext = match lang.as_str() {
        "ts" | "typescript" | "mts" => "ts",
        "tsx" => "tsx",
        "jsx" => "jsx",
        _ => "js",
      };
      let path = file_path.with_extension(ext);
      let mut typescript_config =
        get_typescript_config_builder(fmt_options).build();
      typescript_config.line_width = print_width;
      dprint_plugin_typescript::format_text(
        dprint_plugin_typescript::FormatTextOptions {
          path: &path,
          extension: Some(ext),
          text: text.to_string(),
          config: &typescript_config,
          external_formatter: Some(&create_external_formatter_for_typescript(
            unstable_options,
          )),
        },
      )
    }
    _ => Ok(None),
  }
}

/// A function for formatting embedded code blocks in JavaScript and TypeScript.
fn create_external_formatter_for_typescript(
  unstable_options: &UnstableFmtOptions,
) -> impl Fn(
  &str,
  String,
  &dprint_plugin_typescript::configuration::Configuration,
) -> deno_core::anyhow::Result<Option<String>>
+ use<> {
  let unstable_sql = unstable_options.sql;
  move |lang, text, config| match lang {
    "css" => format_embedded_css(&text, config),
    "html" | "xml" | "svg" => format_embedded_html(lang, &text, config),
    "sql" => {
      if unstable_sql {
        format_embedded_sql(&text, config)
      } else {
        Ok(None)
      }
    }
    _ => Ok(None),
  }
}

/// Formats embedded CSS code blocks in JavaScript and TypeScript.
///
/// Template literal expressions arrive as `@dpr1nt_<n>` placeholder tokens,
/// which lax-css passes through untouched in property, value, selector, and
/// statement positions, so the text can be formatted directly. Properties
/// only CSS expressions, like:
/// ```css
/// margin: 10px;
/// padding: 10px;
/// ```
/// parse as top level declarations without any wrapping.
fn format_embedded_css(
  text: &str,
  config: &dprint_plugin_typescript::configuration::Configuration,
) -> deno_core::anyhow::Result<Option<String>> {
  let lax_css_config = lax_css::configuration::Configuration {
    line_width: config.line_width,
    use_tabs: config.use_tabs,
    indent_width: config.indent_width,
    new_line_kind: dprint_core::configuration::NewLineKind::LineFeed,
    ignore_node_comment_text: "deno-fmt-ignore".to_string(),
    ignore_file_comment_text: "deno-fmt-ignore-file".to_string(),
    single_line: false,
  };
  let Some(formatted) =
    lax_css::format_text(Path::new("embedded.css"), text, &lax_css_config)?
  else {
    return Ok(None);
  };
  let formatted = formatted.trim_end_matches('\n');
  Ok(if formatted == text {
    None
  } else {
    Some(formatted.to_string())
  })
}

/// Formats the embedded HTML code blocks in JavaScript and TypeScript.
fn format_embedded_html(
  _lang: &str,
  text: &str,
  config: &dprint_plugin_typescript::configuration::Configuration,
) -> deno_core::anyhow::Result<Option<String>> {
  let markup_config = lax_markup::configuration::Configuration {
    line_width: config.line_width,
    use_tabs: config.use_tabs,
    indent_width: config.indent_width,
    new_line_kind: dprint_core::configuration::NewLineKind::LineFeed,
    ignore_node_comment_text: "deno-fmt-ignore".to_string(),
    ignore_file_comment_text: "deno-fmt-ignore-file".to_string(),
  };
  // code inside markup that is itself inside a tagged template is kept as
  // written
  let external = |_: &str, _: &str, _: u32| Ok(None);
  // the typescript formatter reindents the template contents when it embeds
  // the result, so the text must be dedented first to make the round trip a
  // fixed point
  let dedented = dedent_embedded(text);
  let Some(formatted) = lax_markup::format_text_with_external(
    Path::new("embedded.html"),
    &dedented,
    &markup_config,
    &external,
  )?
  else {
    return Ok(if dedented == text {
      None
    } else {
      Some(dedented)
    });
  };
  let formatted = formatted.trim_end_matches('\n');
  Ok(if formatted == text {
    None
  } else {
    Some(formatted.to_string())
  })
}

/// Strips the longest common leading whitespace prefix from every non empty
/// line.
fn dedent_embedded(text: &str) -> String {
  let mut common: Option<&str> = None;
  for line in text.split('\n') {
    if line.trim().is_empty() {
      continue;
    }
    let leading = &line[..line.len() - line.trim_start().len()];
    common = Some(match common {
      None => leading,
      Some(prev) => {
        let len = prev
          .as_bytes()
          .iter()
          .zip(leading.as_bytes())
          .take_while(|(a, b)| a == b)
          .count();
        &prev[..len]
      }
    });
  }
  let common = common.unwrap_or("");
  if common.is_empty() {
    return text.to_string();
  }
  text
    .split('\n')
    .map(|line| line.strip_prefix(common).unwrap_or(line))
    .collect::<Vec<_>>()
    .join("\n")
}

/// Formats the embedded SQL code blocks in JavaScript and TypeScript.
fn format_embedded_sql(
  text: &str,
  config: &dprint_plugin_typescript::configuration::Configuration,
) -> deno_core::anyhow::Result<Option<String>> {
  let sql_config = get_resolved_lax_sql_config(
    config.line_width,
    config.use_tabs,
    config.indent_width,
  );
  let Some(formatted) =
    lax_sql::format_text(Path::new("embedded.sql"), text, &sql_config)?
  else {
    return Ok(None);
  };
  let formatted = formatted.trim_end_matches('\n');
  Ok(if formatted == text {
    None
  } else {
    Some(formatted.to_string())
  })
}

fn get_resolved_lax_sql_config(
  line_width: u32,
  use_tabs: bool,
  indent_width: u8,
) -> lax_sql::configuration::Configuration {
  lax_sql::configuration::Configuration {
    line_width,
    use_tabs,
    indent_width,
    new_line_kind: dprint_core::configuration::NewLineKind::LineFeed,
    // matches the previous sqlformat behavior of uppercasing keywords
    keyword_case: lax_sql::configuration::KeywordCase::Upper,
    clause_style: lax_sql::configuration::ClauseStyle::Fill,
    ignore_node_comment_text: "deno-fmt-ignore".to_string(),
    ignore_file_comment_text: "deno-fmt-ignore-file".to_string(),
  }
}

pub fn format_sql(
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  lax_sql::format_text(
    Path::new("file.sql"),
    file_text,
    &get_resolved_lax_sql_config(
      fmt_options.line_width.unwrap_or(80),
      fmt_options.use_tabs.unwrap_or_default(),
      fmt_options.indent_width.unwrap_or(2),
    ),
  )
}

/// Formats a single TS, TSX, JS, JSX, JSONC, JSON, MD, IPYNB or SQL file.
pub fn format_file(
  file_path: &Path,
  file: &FileContents,
  fmt_options: &FmtOptionsConfig,
  unstable_options: &UnstableFmtOptions,
  ext: Option<String>,
) -> Result<Option<String>, AnyError> {
  let ext = ext
    .or_else(|| get_extension(file_path))
    .unwrap_or("ts".to_string());

  let maybe_result = match ext.as_str() {
    "md" | "mkd" | "mkdn" | "mdwn" | "mdown" | "markdown" => {
      format_markdown(&file.text, fmt_options, unstable_options)?
    }
    "json" | "jsonc" => format_json(file_path, &file.text, fmt_options)?,
    "css" | "scss" | "less" => format_css(file_path, &file.text, fmt_options)?,
    "html" | "xml" | "svg" => {
      format_html(file_path, &file.text, fmt_options, unstable_options)?
    }
    "svelte" | "vue" | "astro" | "vto" | "njk" | "mustache" => {
      if unstable_options.component {
        format_html(file_path, &file.text, fmt_options, unstable_options)?
      } else {
        None
      }
    }
    "yml" | "yaml" => format_yaml(&file.text, fmt_options)?,
    "ipynb" => dprint_plugin_jupyter::format_text(
      &file.text,
      |file_path: &Path, file_text: String| {
        let file = FileContents {
          had_bom: false,
          text: file_text.into(),
        };
        format_file(file_path, &file, fmt_options, unstable_options, None)
      },
    )?,
    "sql" => {
      if unstable_options.sql {
        format_sql(&file.text, fmt_options)?
      } else {
        None
      }
    }
    _ => {
      let config = get_resolved_typescript_config(fmt_options);
      dprint_plugin_typescript::format_text(
        dprint_plugin_typescript::FormatTextOptions {
          path: file_path,
          extension: Some(&ext),
          text: file.text.to_string(),
          config: &config,
          external_formatter: Some(&create_external_formatter_for_typescript(
            unstable_options,
          )),
        },
      )?
    }
  };

  Ok(match maybe_result {
    Some(result) => Some(result),
    None if file.had_bom => {
      // return back the text without the BOM
      Some(file.text.to_string())
    }
    None => None,
  })
}

pub fn format_parsed_source(
  parsed_source: &ParsedSource,
  fmt_options: &FmtOptionsConfig,
  unstable_options: &UnstableFmtOptions,
) -> Result<Option<String>, AnyError> {
  dprint_plugin_typescript::format_parsed_source(
    parsed_source,
    &get_resolved_typescript_config(fmt_options),
    Some(&create_external_formatter_for_typescript(unstable_options)),
  )
}

#[async_trait]
trait Formatter {
  async fn handle_files(
    &self,
    paths: Vec<PathBuf>,
    fmt_options: FmtOptionsConfig,
    unstable_options: UnstableFmtOptions,
    incremental_cache: Arc<IncrementalCache>,
    editorconfig_cache: Arc<EditorConfigCache>,
    ext: Option<String>,
  ) -> Result<(), AnyError>;

  fn finish(&self) -> Result<(), AnyError>;
}

/// Returns a per-file [`FmtOptionsConfig`], merging values from
/// `.editorconfig` (lowest priority) under the resolved `base` config
/// (which already incorporates `deno.json` plus CLI flags).
fn resolve_per_file_options(
  base: &FmtOptionsConfig,
  editorconfig_cache: &EditorConfigCache,
  file_path: &Path,
) -> FmtOptionsConfig {
  let props = editorconfig_cache.resolve(file_path);
  if props.is_empty() {
    return base.clone();
  }
  let mut cfg = base.clone();
  props.apply_to(&mut cfg);
  cfg
}

/// Returns the value hashed by the incremental cache for a file. When
/// `.editorconfig` contributes options that differ from the batch-level
/// `base` config, those options are folded into the hashed value so that
/// editing `.editorconfig` invalidates the cached "already formatted"
/// result even when the file body itself is unchanged. When nothing was
/// contributed the file text is hashed as-is, preserving existing cache
/// entries and avoiding an allocation.
fn incremental_cache_text<'a>(
  per_file_options: &FmtOptionsConfig,
  base: &FmtOptionsConfig,
  text: &'a str,
) -> Cow<'a, str> {
  if per_file_options == base {
    Cow::Borrowed(text)
  } else {
    Cow::Owned(format!("{per_file_options:?}\n{text}"))
  }
}

struct CheckFormatter {
  not_formatted_files_count: Arc<AtomicUsize>,
  checked_files_count: Arc<AtomicUsize>,
  fail_fast_found_error: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl CheckFormatter {
  fn new(fail_fast: bool) -> Self {
    Self {
      not_formatted_files_count: Arc::new(AtomicUsize::new(0)),
      checked_files_count: Arc::new(AtomicUsize::new(0)),
      fail_fast_found_error: if fail_fast {
        Some(Arc::new(std::sync::atomic::AtomicBool::new(false)))
      } else {
        None
      },
    }
  }
}

#[async_trait]
impl Formatter for CheckFormatter {
  async fn handle_files(
    &self,
    paths: Vec<PathBuf>,
    fmt_options: FmtOptionsConfig,
    unstable_options: UnstableFmtOptions,
    incremental_cache: Arc<IncrementalCache>,
    editorconfig_cache: Arc<EditorConfigCache>,
    ext: Option<String>,
  ) -> Result<(), AnyError> {
    // prevent threads outputting at the same time
    let output_lock = Arc::new(Mutex::new(0));

    run_parallelized(paths, {
      let not_formatted_files_count = self.not_formatted_files_count.clone();
      let checked_files_count = self.checked_files_count.clone();
      let fail_fast_found_error = self.fail_fast_found_error.clone();
      move |file_path| {
        // Early exit if fail-fast is enabled and we've already found an error
        if let Some(fast) = &fail_fast_found_error
          && fast.load(Ordering::Relaxed)
        {
          return Ok(());
        }

        checked_files_count.fetch_add(1, Ordering::Relaxed);
        let file = read_file_contents(&file_path)?;

        let per_file_options = resolve_per_file_options(
          &fmt_options,
          &editorconfig_cache,
          &file_path,
        );
        let cache_text =
          incremental_cache_text(&per_file_options, &fmt_options, &file.text);

        // skip checking the file if we know it's formatted
        if !file.had_bom
          && incremental_cache.is_file_same(&file_path, &cache_text)
        {
          return Ok(());
        }

        match format_file(
          &file_path,
          &file,
          &per_file_options,
          &unstable_options,
          ext.clone(),
        ) {
          Ok(Some(formatted_text)) => {
            not_formatted_files_count.fetch_add(1, Ordering::Relaxed);
            if let Some(fast) = &fail_fast_found_error {
              fast.store(true, Ordering::Relaxed);
            }
            let _g = output_lock.lock();
            let diff =
              deno_resolver::display::diff(&file.text, &formatted_text);
            info!("");
            info!("{} {}:", colors::bold("from"), file_path.display());
            if file.had_bom {
              info!("  {}", colors::gray("File has strippable UTF-8 BOM."));
            }
            info!("{}", diff);
          }
          Ok(None) => {
            // When checking formatting, only update the incremental cache when
            // the file is the same since we don't bother checking for stable
            // formatting here. Additionally, ensure this is done during check
            // so that CIs that cache the DENO_DIR will get the benefit of
            // incremental formatting
            incremental_cache.update_file(&file_path, &cache_text);
          }
          Err(e) => {
            not_formatted_files_count.fetch_add(1, Ordering::Relaxed);
            if let Some(fast) = &fail_fast_found_error {
              fast.store(true, Ordering::Relaxed);
            }
            let _g = output_lock.lock();
            warn!("Error checking: {}", file_path.to_string_lossy());
            warn!(
              "{}",
              format!("{e}")
                .split('\n')
                .map(|l| {
                  if l.trim().is_empty() {
                    String::new()
                  } else {
                    format!("  {l}")
                  }
                })
                .collect::<Vec<_>>()
                .join("\n")
            );
          }
        }
        Ok(())
      }
    })
    .await?;

    Ok(())
  }

  fn finish(&self) -> Result<(), AnyError> {
    let not_formatted_files_count =
      self.not_formatted_files_count.load(Ordering::Relaxed);
    let checked_files_count = self.checked_files_count.load(Ordering::Relaxed);
    let checked_files_str =
      format!("{} {}", checked_files_count, files_str(checked_files_count));
    if not_formatted_files_count == 0 {
      info!("Checked {}", checked_files_str);
      Ok(())
    } else {
      let not_formatted_files_str = files_str(not_formatted_files_count);
      Err(anyhow!(
        "Found {not_formatted_files_count} not formatted {not_formatted_files_str} in {checked_files_str}",
      ))
    }
  }
}

#[derive(Default)]
struct RealFormatter {
  formatted_files_count: Arc<AtomicUsize>,
  failed_files_count: Arc<AtomicUsize>,
  checked_files_count: Arc<AtomicUsize>,
}

#[async_trait]
impl Formatter for RealFormatter {
  async fn handle_files(
    &self,
    paths: Vec<PathBuf>,
    fmt_options: FmtOptionsConfig,
    unstable_options: UnstableFmtOptions,
    incremental_cache: Arc<IncrementalCache>,
    editorconfig_cache: Arc<EditorConfigCache>,
    ext: Option<String>,
  ) -> Result<(), AnyError> {
    let output_lock = Arc::new(Mutex::new(0)); // prevent threads outputting at the same time

    run_parallelized(paths, {
      let formatted_files_count = self.formatted_files_count.clone();
      let failed_files_count = self.failed_files_count.clone();
      let checked_files_count = self.checked_files_count.clone();
      move |file_path| {
        checked_files_count.fetch_add(1, Ordering::Relaxed);
        let file = read_file_contents(&file_path)?;

        let per_file_options = resolve_per_file_options(
          &fmt_options,
          &editorconfig_cache,
          &file_path,
        );
        let cache_text =
          incremental_cache_text(&per_file_options, &fmt_options, &file.text);

        // skip formatting the file if we know it's formatted
        if !file.had_bom
          && incremental_cache.is_file_same(&file_path, &cache_text)
        {
          return Ok(());
        }

        match format_ensure_stable(&file_path, &file, |file_path, file| {
          format_file(
            file_path,
            file,
            &per_file_options,
            &unstable_options,
            ext.clone(),
          )
        }) {
          Ok(Some(formatted_text)) => {
            incremental_cache.update_file(
              &file_path,
              &incremental_cache_text(
                &per_file_options,
                &fmt_options,
                &formatted_text,
              ),
            );
            write_file_contents(&file_path, &formatted_text)?;
            formatted_files_count.fetch_add(1, Ordering::Relaxed);
            let _g = output_lock.lock();
            info!("{}", file_path.to_string_lossy());
          }
          Ok(None) => {
            incremental_cache.update_file(&file_path, &cache_text);
          }
          Err(e) => {
            failed_files_count.fetch_add(1, Ordering::Relaxed);
            let _g = output_lock.lock();
            log::error!("Error formatting: {}", file_path.to_string_lossy());
            log::error!("   {e}");
          }
        }
        Ok(())
      }
    })
    .await?;
    Ok(())
  }

  fn finish(&self) -> Result<(), AnyError> {
    let formatted_files_count =
      self.formatted_files_count.load(Ordering::Relaxed);
    debug!(
      "Formatted {} {}",
      formatted_files_count,
      files_str(formatted_files_count),
    );

    let failed_files_count = self.failed_files_count.load(Ordering::Relaxed);
    let checked_files_count = self.checked_files_count.load(Ordering::Relaxed);

    if failed_files_count == 0 {
      info!(
        "Checked {} {}",
        checked_files_count,
        files_str(checked_files_count)
      );
      Ok(())
    } else {
      let checked_files_str = format!(
        "{} checked {}",
        checked_files_count,
        files_str(checked_files_count)
      );
      Err(anyhow!(
        "Failed to format {failed_files_count} of {checked_files_str}",
      ))
    }
  }
}

/// When storing any formatted text in the incremental cache, we want
/// to ensure that anything stored when formatted will have itself as
/// the output as well. This is to prevent "double format" issues where
/// a user formats their code locally and it fails on the CI afterwards.
fn format_ensure_stable(
  file_path: &Path,
  file: &FileContents,
  fmt_func: impl Fn(&Path, &FileContents) -> Result<Option<String>, AnyError>,
) -> Result<Option<String>, AnyError> {
  let formatted_text = fmt_func(file_path, file)?;

  match formatted_text {
    Some(mut current_text) => {
      let mut count = 0;
      loop {
        match fmt_func(
          file_path,
          &FileContents {
            had_bom: false,
            text: (&current_text).into(),
          },
        ) {
          Ok(Some(next_pass_text)) => {
            // just in case
            if next_pass_text == current_text {
              return Ok(Some(next_pass_text));
            }
            current_text = next_pass_text;
          }
          Ok(None) => {
            return Ok(Some(current_text));
          }
          Err(err) => {
            bail!(
              concat!(
                "Formatting succeeded initially, but failed when ensuring a ",
                "stable format. This indicates a bug in the formatter where ",
                "the text it produces is not syntactically correct. As a temporary ",
                "workaround you can ignore this file.\n\n{:#}"
              ),
              err,
            )
          }
        }
        count += 1;
        if count == 5 {
          bail!(
            concat!(
              "Formatting not stable. Bailed after {} tries. This indicates a bug ",
              "in the formatter where it formats the file differently each time. As a ",
              "temporary workaround you can ignore this file."
            ),
            count,
          )
        }
      }
    }
    None => Ok(None),
  }
}

/// Format stdin and write result to stdout.
/// Treats input as set by `--ext` flag.
/// Compatible with `--check` flag.
fn format_stdin(
  fmt_flags: &FmtFlags,
  fmt_options: FmtOptions,
  ext: &str,
) -> Result<(), AnyError> {
  let mut source = String::new();
  if stdin().read_to_string(&mut source).is_err() {
    bail!("Failed to read from stdin");
  }
  let file = FileContents {
    had_bom: false,
    text: source.into(),
  };
  let file_path = PathBuf::from(format!("_stdin.{ext}"));
  let formatted_text = format_file(
    &file_path,
    &file,
    &fmt_options.options,
    &fmt_options.unstable,
    None,
  )?;
  if fmt_flags.check {
    #[allow(clippy::print_stdout, reason = "actually want to output")]
    if formatted_text.is_some() {
      println!("Not formatted stdin");
    }
  } else {
    stdout().write_all(
      formatted_text
        .as_ref()
        .map(|t| t.as_bytes())
        .unwrap_or(file.text.as_bytes()),
    )?;
  }
  Ok(())
}

fn files_str(len: usize) -> &'static str {
  if len == 1 { "file" } else { "files" }
}

fn get_typescript_config_builder(
  options: &FmtOptionsConfig,
) -> dprint_plugin_typescript::configuration::ConfigurationBuilder {
  use deno_config::deno_json::*;
  use dprint_plugin_typescript::configuration as dprint_config;

  let mut builder =
    dprint_plugin_typescript::configuration::ConfigurationBuilder::new();
  builder.deno();

  if let Some(use_tabs) = options.use_tabs {
    builder.use_tabs(use_tabs);
  }

  if let Some(line_width) = options.line_width {
    builder.line_width(line_width);
  }

  if let Some(indent_width) = options.indent_width {
    builder.indent_width(indent_width);
  }

  if let Some(single_quote) = options.single_quote
    && single_quote
  {
    builder.quote_style(
      dprint_plugin_typescript::configuration::QuoteStyle::PreferSingle,
    );
  }

  if let Some(semi_colons) = options.semi_colons {
    builder.semi_colons(match semi_colons {
      true => dprint_plugin_typescript::configuration::SemiColons::Prefer,
      false => dprint_plugin_typescript::configuration::SemiColons::Asi,
    });
  }

  if let Some(quote_props) = options.quote_props {
    builder.quote_props(match quote_props {
      QuoteProps::AsNeeded => dprint_config::QuoteProps::AsNeeded,
      QuoteProps::Consistent => dprint_config::QuoteProps::Consistent,
      QuoteProps::Preserve => dprint_config::QuoteProps::Preserve,
    });
  }

  if let Some(new_line_kind) = options.new_line_kind {
    builder.new_line_kind(match new_line_kind {
      NewLineKind::Auto => dprint_core::configuration::NewLineKind::Auto,
      NewLineKind::CarriageReturnLineFeed => {
        dprint_core::configuration::NewLineKind::CarriageReturnLineFeed
      }
      NewLineKind::LineFeed => {
        dprint_core::configuration::NewLineKind::LineFeed
      }
      NewLineKind::System => {
        if cfg!(windows) {
          dprint_core::configuration::NewLineKind::CarriageReturnLineFeed
        } else {
          dprint_core::configuration::NewLineKind::LineFeed
        }
      }
    });
  }

  if let Some(use_braces) = options.use_braces {
    builder.use_braces(match use_braces {
      UseBraces::Always => dprint_config::UseBraces::Always,
      UseBraces::Maintain => dprint_config::UseBraces::Maintain,
      UseBraces::PreferNone => dprint_config::UseBraces::PreferNone,
      UseBraces::WhenNotSingleLine => {
        dprint_config::UseBraces::WhenNotSingleLine
      }
    });
  }

  if let Some(brace_position) = options.brace_position {
    builder.brace_position(match brace_position {
      BracePosition::Maintain => dprint_config::BracePosition::Maintain,
      BracePosition::NextLine => dprint_config::BracePosition::NextLine,
      BracePosition::SameLine => dprint_config::BracePosition::SameLine,
      BracePosition::SameLineUnlessHanging => {
        dprint_config::BracePosition::SameLineUnlessHanging
      }
    });
  }

  if let Some(single_body_position) = options.single_body_position {
    builder.single_body_position(match single_body_position {
      SingleBodyPosition::Maintain => {
        dprint_config::SameOrNextLinePosition::Maintain
      }
      SingleBodyPosition::NextLine => {
        dprint_config::SameOrNextLinePosition::NextLine
      }
      SingleBodyPosition::SameLine => {
        dprint_config::SameOrNextLinePosition::SameLine
      }
    });
  }

  if let Some(next_control_flow_position) = options.next_control_flow_position {
    builder.next_control_flow_position(match next_control_flow_position {
      NextControlFlowPosition::Maintain => {
        dprint_config::NextControlFlowPosition::Maintain
      }
      NextControlFlowPosition::NextLine => {
        dprint_config::NextControlFlowPosition::NextLine
      }
      NextControlFlowPosition::SameLine => {
        dprint_config::NextControlFlowPosition::SameLine
      }
    });
  }

  if let Some(trailing_commas) = options.trailing_commas {
    builder.trailing_commas(match trailing_commas {
      TrailingCommas::Always => dprint_config::TrailingCommas::Always,
      TrailingCommas::Never => dprint_config::TrailingCommas::Never,
      TrailingCommas::OnlyMultiLine => {
        dprint_config::TrailingCommas::OnlyMultiLine
      }
    });
  }

  if let Some(operator_position) = options.operator_position {
    let option = match operator_position {
      OperatorPosition::Maintain => dprint_config::OperatorPosition::Maintain,
      OperatorPosition::NextLine => dprint_config::OperatorPosition::NextLine,
      OperatorPosition::SameLine => dprint_config::OperatorPosition::SameLine,
    };
    // Because Deno's defaults are set at AST specific options, we need to
    // set them to AST specific options to override them.
    builder.binary_expression_operator_position(option);
    builder.conditional_expression_operator_position(option);
    builder.conditional_type_operator_position(option);
  }

  if let Some(jsx_bracket_position) = options.jsx_bracket_position {
    builder.jsx_bracket_position(match jsx_bracket_position {
      BracketPosition::Maintain => {
        dprint_config::SameOrNextLinePosition::Maintain
      }
      BracketPosition::NextLine => {
        dprint_config::SameOrNextLinePosition::NextLine
      }
      BracketPosition::SameLine => {
        dprint_config::SameOrNextLinePosition::SameLine
      }
    });
  }

  if let Some(jsx_force_new_lines_surrounding_content) =
    options.jsx_force_new_lines_surrounding_content
  {
    builder.jsx_force_new_lines_surrounding_content(
      jsx_force_new_lines_surrounding_content,
    );
  }

  if let Some(jsx_multi_line_parens) = options.jsx_multi_line_parens {
    builder.jsx_multi_line_parens(match jsx_multi_line_parens {
      MultiLineParens::Always => dprint_config::JsxMultiLineParens::Always,
      MultiLineParens::Never => dprint_config::JsxMultiLineParens::Never,
      MultiLineParens::Prefer => dprint_config::JsxMultiLineParens::Prefer,
    });
  }

  if let Some(type_literal_separator_kind) = options.type_literal_separator_kind
  {
    builder.type_literal_separator_kind(match type_literal_separator_kind {
      SeparatorKind::Comma => dprint_config::SemiColonOrComma::Comma,
      SeparatorKind::SemiColon => dprint_config::SemiColonOrComma::SemiColon,
    });
  }

  if let Some(space_around) = options.space_around {
    builder.space_around(space_around);
  }

  if let Some(space_surrounding_properties) =
    options.space_surrounding_properties
  {
    builder.space_surrounding_properties(space_surrounding_properties);
    builder.import_declaration_space_surrounding_named_imports(
      space_surrounding_properties,
    );
    builder.export_declaration_space_surrounding_named_exports(
      space_surrounding_properties,
    );
  }

  builder
}

fn get_resolved_typescript_config(
  options: &FmtOptionsConfig,
) -> dprint_plugin_typescript::configuration::Configuration {
  get_typescript_config_builder(options).build()
}

fn get_resolved_markdown_config(
  options: &FmtOptionsConfig,
) -> dprint_plugin_markdown::configuration::Configuration {
  let mut builder =
    dprint_plugin_markdown::configuration::ConfigurationBuilder::new();

  builder.deno();

  if let Some(line_width) = options.line_width {
    builder.line_width(line_width);
  }

  if let Some(prose_wrap) = options.prose_wrap {
    builder.text_wrap(match prose_wrap {
      ProseWrap::Always => {
        dprint_plugin_markdown::configuration::TextWrap::Always
      }
      ProseWrap::Never => {
        dprint_plugin_markdown::configuration::TextWrap::Never
      }
      ProseWrap::Preserve => {
        dprint_plugin_markdown::configuration::TextWrap::Maintain
      }
    });
  }

  if let Some(new_line_kind) = options.new_line_kind {
    builder.new_line_kind(match new_line_kind {
      NewLineKind::Auto => dprint_core::configuration::NewLineKind::Auto,
      NewLineKind::CarriageReturnLineFeed => {
        dprint_core::configuration::NewLineKind::CarriageReturnLineFeed
      }
      NewLineKind::LineFeed => {
        dprint_core::configuration::NewLineKind::LineFeed
      }
      NewLineKind::System => {
        if cfg!(windows) {
          dprint_core::configuration::NewLineKind::CarriageReturnLineFeed
        } else {
          dprint_core::configuration::NewLineKind::LineFeed
        }
      }
    });
  }

  builder.build()
}

fn get_resolved_json_config(
  options: &FmtOptionsConfig,
) -> dprint_plugin_json::configuration::Configuration {
  use deno_config::deno_json::JsonTrailingCommaKind;
  use dprint_plugin_json::configuration::TrailingCommaKind;

  let mut builder =
    dprint_plugin_json::configuration::ConfigurationBuilder::new();

  builder.deno();
  if let Some(json_trailing_commas) = options.json_trailing_commas {
    builder.trailing_commas(match json_trailing_commas {
      JsonTrailingCommaKind::Always => TrailingCommaKind::Always,
      JsonTrailingCommaKind::Jsonc => TrailingCommaKind::Jsonc,
      JsonTrailingCommaKind::Maintain => TrailingCommaKind::Maintain,
      JsonTrailingCommaKind::Never => TrailingCommaKind::Never,
    });
  }

  if let Some(use_tabs) = options.use_tabs {
    builder.use_tabs(use_tabs);
  }

  if let Some(line_width) = options.line_width {
    builder.line_width(line_width);
  }

  if let Some(indent_width) = options.indent_width {
    builder.indent_width(indent_width);
  }

  if let Some(new_line_kind) = options.new_line_kind {
    builder.new_line_kind(match new_line_kind {
      NewLineKind::Auto => dprint_core::configuration::NewLineKind::Auto,
      NewLineKind::CarriageReturnLineFeed => {
        dprint_core::configuration::NewLineKind::CarriageReturnLineFeed
      }
      NewLineKind::LineFeed => {
        dprint_core::configuration::NewLineKind::LineFeed
      }
      NewLineKind::System => {
        if cfg!(windows) {
          dprint_core::configuration::NewLineKind::CarriageReturnLineFeed
        } else {
          dprint_core::configuration::NewLineKind::LineFeed
        }
      }
    });
  }

  builder.build()
}

fn get_resolved_lax_markup_config(
  options: &FmtOptionsConfig,
) -> lax_markup::configuration::Configuration {
  lax_markup::configuration::Configuration {
    line_width: options.line_width.unwrap_or(80),
    use_tabs: options.use_tabs.unwrap_or_default(),
    indent_width: options.indent_width.unwrap_or(2),
    new_line_kind: dprint_core::configuration::NewLineKind::LineFeed,
    ignore_node_comment_text: "deno-fmt-ignore".to_string(),
    ignore_file_comment_text: "deno-fmt-ignore-file".to_string(),
  }
}

fn get_resolved_yaml_config(
  options: &FmtOptionsConfig,
) -> pretty_yaml::config::FormatOptions {
  use pretty_yaml::config::*;

  let layout_options = LayoutOptions {
    print_width: options.line_width.unwrap_or(80) as usize,
    indent_width: options.indent_width.unwrap_or(2) as usize,
    line_break: LineBreak::Lf,
  };

  let language_options = LanguageOptions {
    quotes: if let Some(true) = options.single_quote {
      Quotes::PreferSingle
    } else {
      Quotes::PreferDouble
    },
    trailing_comma: true,
    format_comments: false,
    indent_block_sequence_in_map: true,
    brace_spacing: true,
    bracket_spacing: false,
    dash_spacing: DashSpacing::OneSpace,
    prefer_single_line: false,
    flow_sequence_prefer_single_line: None,
    flow_map_prefer_single_line: None,
    trim_trailing_whitespaces: true,
    trim_trailing_zero: false,
    ignore_comment_directive: "deno-fmt-ignore".into(),
  };

  FormatOptions {
    layout: layout_options,
    language: language_options,
  }
}

pub struct FileContents<'a> {
  pub text: Cow<'a, str>,
  pub had_bom: bool,
}

fn read_file_contents(file_path: &Path) -> Result<FileContents<'_>, AnyError> {
  let file_bytes = fs::read(file_path)
    .with_context(|| format!("Error reading {}", file_path.display()))?;
  let had_bom = file_bytes.starts_with(&[0xEF, 0xBB, 0xBF]);

  let charset =
    deno_media_type::encoding::detect_charset_local_file(&file_bytes);
  let text = deno_media_type::encoding::decode_owned_source(
    charset,
    file_bytes.to_vec(),
  )
  .with_context(|| {
    anyhow!("{} is not a valid UTF-8 file", file_path.display())
  })?;

  Ok(FileContents {
    text: Cow::Owned(text),
    had_bom,
  })
}

fn write_file_contents(
  file_path: &Path,
  file_contents: &str,
) -> Result<(), AnyError> {
  Ok(fs::write(file_path, file_contents)?)
}

pub async fn run_parallelized<F>(
  file_paths: Vec<PathBuf>,
  f: F,
) -> Result<(), AnyError>
where
  F: FnOnce(PathBuf) -> Result<(), AnyError> + Send + 'static + Clone,
{
  let handles = file_paths.iter().map(|file_path| {
    let f = f.clone();
    let file_path = file_path.clone();
    spawn_blocking(move || f(file_path))
  });
  let join_results = futures::future::join_all(handles).await;

  // find the tasks that panicked and let the user know which files
  let panic_file_paths = join_results
    .iter()
    .enumerate()
    .filter_map(|(i, join_result)| {
      join_result
        .as_ref()
        .err()
        .map(|_| file_paths[i].to_string_lossy())
    })
    .collect::<Vec<_>>();
  if !panic_file_paths.is_empty() {
    panic!("Panic formatting: {}", panic_file_paths.join(", "))
  }

  // check for any errors and if so return the first one
  let mut errors = join_results.into_iter().filter_map(|join_result| {
    join_result
      .ok()
      .and_then(|handle_result| handle_result.err())
  });

  match errors.next() {
    Some(e) => Err(e),
    _ => Ok(()),
  }
}

/// This function is similar to is_supported_ext but adds additional extensions
/// supported by `deno fmt`.
fn is_supported_ext_fmt(path: &Path) -> bool {
  get_extension(path).is_some_and(|ext| {
    matches!(
      ext.as_str(),
      "ts"
        | "tsx"
        | "js"
        | "jsx"
        | "cjs"
        | "cts"
        | "mjs"
        | "mts"
        | "json"
        | "jsonc"
        | "css"
        | "scss"
        | "less"
        | "html"
        | "svelte"
        | "vue"
        | "astro"
        | "vto"
        | "njk"
        | "md"
        | "mkd"
        | "mkdn"
        | "mdwn"
        | "mdown"
        | "markdown"
        | "yml"
        | "yaml"
        | "ipynb"
        | "sql"
        | "xml"
        | "svg"
        | "mustache"
    )
  })
}

#[cfg(test)]
mod test {
  use deno_config::deno_json::JsonTrailingCommaKind;
  use test_util::assert_starts_with;

  use super::*;

  #[test]
  fn test_is_supported_ext_fmt() {
    assert!(!is_supported_ext_fmt(Path::new("tests/subdir/redirects")));
    assert!(is_supported_ext_fmt(Path::new("README.md")));
    assert!(is_supported_ext_fmt(Path::new("readme.MD")));
    assert!(is_supported_ext_fmt(Path::new("readme.mkd")));
    assert!(is_supported_ext_fmt(Path::new("readme.mkdn")));
    assert!(is_supported_ext_fmt(Path::new("readme.mdwn")));
    assert!(is_supported_ext_fmt(Path::new("readme.mdown")));
    assert!(is_supported_ext_fmt(Path::new("readme.markdown")));
    assert!(is_supported_ext_fmt(Path::new("lib/typescript.d.ts")));
    assert!(is_supported_ext_fmt(Path::new("testdata/run/001_hello.js")));
    assert!(is_supported_ext_fmt(Path::new("testdata/run/002_hello.ts")));
    assert!(is_supported_ext_fmt(Path::new("foo.jsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.tsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.TS")));
    assert!(is_supported_ext_fmt(Path::new("foo.TSX")));
    assert!(is_supported_ext_fmt(Path::new("foo.JS")));
    assert!(is_supported_ext_fmt(Path::new("foo.JSX")));
    assert!(is_supported_ext_fmt(Path::new("foo.mjs")));
    assert!(!is_supported_ext_fmt(Path::new("foo.mjsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.jsonc")));
    assert!(is_supported_ext_fmt(Path::new("foo.JSONC")));
    assert!(is_supported_ext_fmt(Path::new("foo.json")));
    assert!(is_supported_ext_fmt(Path::new("foo.JsON")));
    assert!(is_supported_ext_fmt(Path::new("foo.css")));
    assert!(is_supported_ext_fmt(Path::new("foo.Css")));
    assert!(is_supported_ext_fmt(Path::new("foo.scss")));
    assert!(is_supported_ext_fmt(Path::new("foo.SCSS")));
    assert!(!is_supported_ext_fmt(Path::new("foo.sass")));
    assert!(!is_supported_ext_fmt(Path::new("foo.Sass")));
    assert!(is_supported_ext_fmt(Path::new("foo.less")));
    assert!(is_supported_ext_fmt(Path::new("foo.LeSS")));
    assert!(is_supported_ext_fmt(Path::new("foo.html")));
    assert!(is_supported_ext_fmt(Path::new("foo.HTML")));
    assert!(is_supported_ext_fmt(Path::new("foo.svelte")));
    assert!(is_supported_ext_fmt(Path::new("foo.Svelte")));
    assert!(is_supported_ext_fmt(Path::new("foo.vue")));
    assert!(is_supported_ext_fmt(Path::new("foo.VUE")));
    assert!(is_supported_ext_fmt(Path::new("foo.astro")));
    assert!(is_supported_ext_fmt(Path::new("foo.AsTrO")));
    assert!(is_supported_ext_fmt(Path::new("foo.vto")));
    assert!(is_supported_ext_fmt(Path::new("foo.Vto")));
    assert!(is_supported_ext_fmt(Path::new("foo.njk")));
    assert!(is_supported_ext_fmt(Path::new("foo.NJk")));
    assert!(is_supported_ext_fmt(Path::new("foo.yml")));
    assert!(is_supported_ext_fmt(Path::new("foo.Yml")));
    assert!(is_supported_ext_fmt(Path::new("foo.yaml")));
    assert!(is_supported_ext_fmt(Path::new("foo.YaML")));
    assert!(is_supported_ext_fmt(Path::new("foo.ipynb")));
    assert!(is_supported_ext_fmt(Path::new("foo.sql")));
    assert!(is_supported_ext_fmt(Path::new("foo.Sql")));
    assert!(is_supported_ext_fmt(Path::new("foo.sQl")));
    assert!(is_supported_ext_fmt(Path::new("foo.sqL")));
    assert!(is_supported_ext_fmt(Path::new("foo.SQL")));
  }

  #[test]
  fn test_format_ensure_stable_unstable_format() {
    let err = format_ensure_stable(
      Path::new("mod.ts"),
      &FileContents {
        had_bom: false,
        text: "1".into(),
      },
      |_, file| Ok(Some(format!("1{}", file.text))),
    )
    .unwrap_err();
    assert_starts_with!(
      err.to_string(),
      "Formatting not stable. Bailed after 5 tries."
    );
  }

  #[test]
  fn test_format_ensure_stable_error_first() {
    let err = format_ensure_stable(
      Path::new("mod.ts"),
      &FileContents {
        had_bom: false,
        text: "1".into(),
      },
      |_, _| bail!("Error formatting."),
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "Error formatting.");
  }

  #[test]
  fn test_format_ensure_stable_error_second() {
    let err = format_ensure_stable(
      Path::new("mod.ts"),
      &FileContents {
        had_bom: false,
        text: "1".into(),
      },
      |_, file| {
        if file.text == "1" {
          Ok(Some("11".to_string()))
        } else {
          bail!("Error formatting.")
        }
      },
    )
    .unwrap_err();
    assert_starts_with!(
      err.to_string(),
      "Formatting succeeded initially, but failed when"
    );
  }

  #[test]
  fn test_format_stable_after_two() {
    let result = format_ensure_stable(
      Path::new("mod.ts"),
      &FileContents {
        had_bom: false,
        text: "1".into(),
      },
      |_, file| {
        if file.text == "1" {
          Ok(Some("11".to_string()))
        } else if file.text == "11" {
          Ok(None)
        } else {
          unreachable!();
        }
      },
    )
    .unwrap();

    assert_eq!(result, Some("11".to_string()));
  }

  #[test]
  fn test_single_quote_true_prefers_single_quote() {
    let file_text = format_file(
      Path::new("test.ts"),
      &FileContents {
        had_bom: false,
        text: "console.log(\"there's\");\nconsole.log('hi');\nconsole.log(\"bye\")\n".into(),
      },
      &FmtOptionsConfig {
        single_quote: Some(true),
        ..Default::default()
      },
      &UnstableFmtOptions::default(),
      None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
      file_text,
      // should use double quotes for the string with a single quote
      "console.log(\"there's\");\nconsole.log('hi');\nconsole.log('bye');\n",
    );
  }

  #[test]
  fn test_formated_removes_utf8_bom() {
    let file_text = format_file(
      Path::new("test.ts"),
      &FileContents {
        had_bom: true,
        text: "let a = 1;".into(),
      },
      &FmtOptionsConfig {
        single_quote: Some(true),
        ..Default::default()
      },
      &UnstableFmtOptions::default(),
      None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(file_text, "let a = 1;\n",);
  }

  #[test]
  fn test_jsonc_does_not_add_trailing_commas_by_default() {
    let file_text = format_file(
      Path::new("test.jsonc"),
      &FileContents {
        had_bom: false,
        text: r#"{
  "a": 1,
  "b": 2
}
"#
        .into(),
      },
      &FmtOptionsConfig::default(),
      &UnstableFmtOptions::default(),
      None,
    )
    .unwrap();
    assert_eq!(file_text, None);
  }

  #[test]
  fn test_json_does_not_add_trailing_commas() {
    let file_text = format_file(
      Path::new("test.json"),
      &FileContents {
        had_bom: false,
        text: r#"{
  "a": 1,
  "b": 2
}
"#
        .into(),
      },
      &FmtOptionsConfig::default(),
      &UnstableFmtOptions::default(),
      None,
    )
    .unwrap();
    assert_eq!(file_text, None);
  }

  #[test]
  fn test_jsonc_trailing_commas_can_be_disabled() {
    let file_text = format_file(
      Path::new("test.jsonc"),
      &FileContents {
        had_bom: false,
        text: r#"{
  "a": 1,
  "b": 2
}
"#
        .into(),
      },
      &FmtOptionsConfig {
        json_trailing_commas: Some(JsonTrailingCommaKind::Never),
        ..Default::default()
      },
      &UnstableFmtOptions::default(),
      None,
    )
    .unwrap();
    assert_eq!(file_text, None);
  }

  #[test]
  fn test_json_trailing_commas_can_be_enabled() {
    let file_text = format_file(
      Path::new("test.json"),
      &FileContents {
        had_bom: false,
        text: r#"{
  "a": 1,
  "b": 2
}
"#
        .into(),
      },
      &FmtOptionsConfig {
        json_trailing_commas: Some(JsonTrailingCommaKind::Always),
        ..Default::default()
      },
      &UnstableFmtOptions::default(),
      None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
      file_text,
      r#"{
  "a": 1,
  "b": 2,
}
"#
    );
  }
}
