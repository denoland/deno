// Copyright 2018-2025 the Deno authors. MIT license.

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
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::unsync::spawn_blocking;
use deno_core::url::Url;
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
            let _ = watcher_communicator
              .watch_paths(paths_with_options.paths.clone());
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
    Box::new(CheckFormatter::default())
  } else {
    Box::new(RealFormatter::default())
  };
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
          | "sass"
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
          "css" | "scss" | "sass" | "less" => {
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
  let formatted_str = malva::format_text(
    file_text,
    malva::detect_syntax(file_path).unwrap_or(malva::Syntax::Css),
    &get_resolved_malva_config(fmt_options),
  )
  .map_err(AnyError::from)?;

  Ok(if formatted_str == file_text {
    None
  } else {
    Some(formatted_str)
  })
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
  let format_result = markup_fmt::format_text(
    file_text,
    markup_fmt::detect_language(file_path)
      .unwrap_or(markup_fmt::Language::Html),
    &get_resolved_markup_fmt_config(fmt_options),
    |text, hints| {
      let mut file_name =
        file_path.file_name().expect("missing file name").to_owned();
      file_name.push(".");
      file_name.push(hints.ext);
      let path = file_path.with_file_name(file_name);
      match hints.ext {
        "css" | "scss" | "sass" | "less" => {
          let mut malva_config = get_resolved_malva_config(fmt_options);
          malva_config.layout.print_width = hints.print_width;
          if hints.attr {
            malva_config.language.quotes =
              if let Some(true) = fmt_options.single_quote {
                malva::config::Quotes::AlwaysDouble
              } else {
                malva::config::Quotes::AlwaysSingle
              };
            malva_config.language.single_line_top_level_declarations = true;
          }
          malva::format_text(
            text,
            malva::detect_syntax(path).unwrap_or(malva::Syntax::Css),
            &malva_config,
          )
          .map(Cow::from)
          .map_err(AnyError::from)
        }
        "json" | "jsonc" => {
          let mut json_config = get_resolved_json_config(fmt_options);
          json_config.line_width = hints.print_width as u32;
          dprint_plugin_json::format_text(&path, text, &json_config).map(
            |formatted| {
              if let Some(formatted) = formatted {
                Cow::from(formatted)
              } else {
                Cow::from(text)
              }
            },
          )
        }
        _ => {
          let mut typescript_config_builder =
            get_typescript_config_builder(fmt_options);
          typescript_config_builder.file_indent_level(hints.indent_level);
          let mut typescript_config = typescript_config_builder.build();
          typescript_config.line_width = hints.print_width as u32;
          dprint_plugin_typescript::format_text(
            dprint_plugin_typescript::FormatTextOptions {
              path: &path,
              extension: None,
              text: text.to_string(),
              config: &typescript_config,
              external_formatter: Some(
                &create_external_formatter_for_typescript(unstable_options),
              ),
            },
          )
          .map(|formatted| {
            if let Some(formatted) = formatted {
              Cow::from(formatted)
            } else {
              Cow::from(text)
            }
          })
        }
      }
    },
  )
  .map_err(|error| match error {
    markup_fmt::FormatError::Syntax(error) => {
      fn inner(
        error: &markup_fmt::SyntaxError,
        file_path: &Path,
      ) -> Option<String> {
        let url = Url::from_file_path(file_path).ok()?;

        let error_msg = format!(
          "Syntax error ({}) at {}:{}:{}\n",
          error.kind,
          url.as_str(),
          error.line,
          error.column
        );
        Some(error_msg)
      }

      if let Some(error_msg) = inner(&error, file_path) {
        AnyError::msg(error_msg)
      } else {
        AnyError::from(error)
      }
    }
    markup_fmt::FormatError::External(errors) => {
      let last = errors.len() - 1;
      AnyError::msg(
        errors
          .into_iter()
          .enumerate()
          .map(|(i, error)| {
            if i == last {
              format!("{error}")
            } else {
              format!("{error}\n\n")
            }
          })
          .collect::<String>(),
      )
    }
  });

  let formatted_str = format_result?;

  Ok(if formatted_str == file_text {
    None
  } else {
    Some(formatted_str)
  })
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
/// This function supports properties only CSS expressions, like:
/// ```css
/// margin: 10px;
/// padding: 10px;
/// ```
///
/// To support this scenario, this function first wraps the text with `a { ... }`,
/// and then strips it off after formatting with malva.
fn format_embedded_css(
  text: &str,
  config: &dprint_plugin_typescript::configuration::Configuration,
) -> deno_core::anyhow::Result<Option<String>> {
  use malva::config;
  let options = config::FormatOptions {
    layout: config::LayoutOptions {
      indent_width: config.indent_width as usize,
      use_tabs: config.use_tabs,
      print_width: config.line_width as usize,
      line_break: match config.new_line_kind {
        dprint_core::configuration::NewLineKind::LineFeed => {
          config::LineBreak::Lf
        }
        dprint_core::configuration::NewLineKind::CarriageReturnLineFeed => {
          config::LineBreak::Crlf
        }
        _ => config::LineBreak::Lf,
      },
    },
    language: config::LanguageOptions {
      hex_case: config::HexCase::Lower,
      hex_color_length: None,
      quotes: config::Quotes::AlwaysDouble,
      operator_linebreak: config::OperatorLineBreak::After,
      block_selector_linebreak: config::BlockSelectorLineBreak::Consistent,
      omit_number_leading_zero: false,
      trailing_comma: false,
      format_comments: false,
      align_comments: true,
      linebreak_in_pseudo_parens: false,
      declaration_order: None,
      single_line_block_threshold: None,
      keyframe_selector_notation: None,
      attr_value_quotes: config::AttrValueQuotes::Always,
      prefer_single_line: false,
      selectors_prefer_single_line: None,
      function_args_prefer_single_line: None,
      sass_content_at_rule_prefer_single_line: None,
      sass_include_at_rule_prefer_single_line: None,
      sass_map_prefer_single_line: None,
      sass_module_config_prefer_single_line: None,
      sass_params_prefer_single_line: None,
      less_import_options_prefer_single_line: None,
      less_mixin_args_prefer_single_line: None,
      less_mixin_params_prefer_single_line: None,
      single_line_top_level_declarations: false,
      selector_override_comment_directive: "malva-selector-override".into(),
      ignore_comment_directive: "malva-ignore".into(),
      ignore_file_comment_directive: "malva-ignore-file".into(),
      declaration_order_group_by:
        config::DeclarationOrderGroupBy::NonDeclaration,
    },
  };
  // Wraps the text in a css block of `a { ... ;}`
  // to make it valid css
  // Note: We choose LESS for the syntax because it allows us to use
  // @variable for both property values and mixins, which is convenient
  // for handling placeholders used as both properties and mixins.
  let text = malva::format_text(
    &format!("a{{\n{}\n;}}", text),
    malva::Syntax::Less,
    &options,
  )?;
  let mut buf = vec![];
  for (i, l) in text.lines().enumerate() {
    // skip the first line (a {)
    if i == 0 {
      continue;
    }
    // skip the last line (})
    if l.starts_with("}") {
      continue;
    }
    let mut chars = l.chars();

    // indent width option is disregarded when use tabs is true since
    // only one tab will be inserted when indented once
    // https://malva.netlify.app/config/indent-width.html
    let indent_width = if config.use_tabs {
      1
    } else {
      config.indent_width as usize
    };

    // drop the indentation
    for _ in 0..indent_width {
      chars.next();
    }

    buf.push(chars.as_str());
  }
  Ok(Some(buf.join("\n").to_string()))
}

/// Formats the embedded HTML code blocks in JavaScript and TypeScript.
fn format_embedded_html(
  lang: &str,
  text: &str,
  config: &dprint_plugin_typescript::configuration::Configuration,
) -> deno_core::anyhow::Result<Option<String>> {
  use markup_fmt::config;

  let language = match lang {
    "xml" | "svg" => markup_fmt::Language::Xml,
    _ => markup_fmt::Language::Html,
  };

  let options = config::FormatOptions {
    layout: config::LayoutOptions {
      indent_width: config.indent_width as usize,
      use_tabs: config.use_tabs,
      print_width: config.line_width as usize,
      line_break: match config.new_line_kind {
        dprint_core::configuration::NewLineKind::LineFeed => {
          config::LineBreak::Lf
        }
        dprint_core::configuration::NewLineKind::CarriageReturnLineFeed => {
          config::LineBreak::Crlf
        }
        _ => config::LineBreak::Lf,
      },
    },
    language: config::LanguageOptions {
      quotes: config::Quotes::Double,
      format_comments: false,
      script_indent: false,
      html_script_indent: None,
      vue_script_indent: None,
      svelte_script_indent: None,
      astro_script_indent: None,
      style_indent: false,
      html_style_indent: None,
      vue_style_indent: None,
      svelte_style_indent: None,
      astro_style_indent: None,
      closing_bracket_same_line: false,
      closing_tag_line_break_for_empty:
        config::ClosingTagLineBreakForEmpty::Fit,
      max_attrs_per_line: None,
      prefer_attrs_single_line: false,
      single_attr_same_line: false,
      html_normal_self_closing: None,
      html_void_self_closing: None,
      component_self_closing: None,
      svg_self_closing: None,
      mathml_self_closing: None,
      whitespace_sensitivity: config::WhitespaceSensitivity::Css,
      component_whitespace_sensitivity: None,
      doctype_keyword_case: config::DoctypeKeywordCase::Upper,
      v_bind_style: None,
      v_on_style: None,
      v_for_delimiter_style: None,
      v_slot_style: None,
      component_v_slot_style: None,
      default_v_slot_style: None,
      named_v_slot_style: None,
      v_bind_same_name_short_hand: None,
      strict_svelte_attr: false,
      svelte_attr_shorthand: None,
      svelte_directive_shorthand: None,
      astro_attr_shorthand: None,
      script_formatter: None,
      ignore_comment_directive: "deno-fmt-ignore".into(),
      ignore_file_comment_directive: "deno-fmt-ignore-file".into(),
    },
  };
  let text = markup_fmt::format_text(text, language, &options, |code, _| {
    Ok::<_, std::convert::Infallible>(code.into())
  })?;
  Ok(Some(text.to_string()))
}

/// Formats the embedded SQL code blocks in JavaScript and TypeScript.
fn format_embedded_sql(
  text: &str,
  config: &dprint_plugin_typescript::configuration::Configuration,
) -> deno_core::anyhow::Result<Option<String>> {
  Ok(Some(format_sql_text(
    text,
    config.use_tabs,
    config.indent_width,
  )))
}

fn format_sql_text(text: &str, use_tabs: bool, indent_width: u8) -> String {
  let mut text = sqlformat::format(
    text,
    &sqlformat::QueryParams::None,
    &sqlformat::FormatOptions {
      ignore_case_convert: None,
      indent: if use_tabs {
        sqlformat::Indent::Tabs
      } else {
        sqlformat::Indent::Spaces(indent_width)
      },
      // leave one blank line between queries.
      lines_between_queries: 2,
      uppercase: Some(true),
    },
  );
  // Add single new line to the end of text.
  text.push('\n');
  text
}

pub fn format_sql(
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  let ignore_file = file_text
    .lines()
    .take_while(|line| line.starts_with("--"))
    .any(|line| {
      line
        .strip_prefix("--")
        .unwrap()
        .trim()
        .starts_with("deno-fmt-ignore-file")
    });

  if ignore_file {
    return Ok(None);
  }

  let formatted_str = format_sql_text(
    file_text,
    fmt_options.use_tabs.unwrap_or_default(),
    fmt_options.indent_width.unwrap_or(2),
  );

  Ok(if formatted_str == file_text {
    None
  } else {
    Some(formatted_str)
  })
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
    "css" | "scss" | "sass" | "less" => {
      format_css(file_path, &file.text, fmt_options)?
    }
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
    ext: Option<String>,
  ) -> Result<(), AnyError>;

  fn finish(&self) -> Result<(), AnyError>;
}

#[derive(Default)]
struct CheckFormatter {
  not_formatted_files_count: Arc<AtomicUsize>,
  checked_files_count: Arc<AtomicUsize>,
}

#[async_trait]
impl Formatter for CheckFormatter {
  async fn handle_files(
    &self,
    paths: Vec<PathBuf>,
    fmt_options: FmtOptionsConfig,
    unstable_options: UnstableFmtOptions,
    incremental_cache: Arc<IncrementalCache>,
    ext: Option<String>,
  ) -> Result<(), AnyError> {
    // prevent threads outputting at the same time
    let output_lock = Arc::new(Mutex::new(0));

    run_parallelized(paths, {
      let not_formatted_files_count = self.not_formatted_files_count.clone();
      let checked_files_count = self.checked_files_count.clone();
      move |file_path| {
        checked_files_count.fetch_add(1, Ordering::Relaxed);
        let file = read_file_contents(&file_path)?;

        // skip checking the file if we know it's formatted
        if !file.had_bom
          && incremental_cache.is_file_same(&file_path, &file.text)
        {
          return Ok(());
        }

        match format_file(
          &file_path,
          &file,
          &fmt_options,
          &unstable_options,
          ext.clone(),
        ) {
          Ok(Some(formatted_text)) => {
            not_formatted_files_count.fetch_add(1, Ordering::Relaxed);
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
            incremental_cache.update_file(&file_path, &file.text);
          }
          Err(e) => {
            not_formatted_files_count.fetch_add(1, Ordering::Relaxed);
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

        // skip formatting the file if we know it's formatted
        if !file.had_bom
          && incremental_cache.is_file_same(&file_path, &file.text)
        {
          return Ok(());
        }

        match format_ensure_stable(&file_path, &file, |file_path, file| {
          format_file(
            file_path,
            file,
            &fmt_options,
            &unstable_options,
            ext.clone(),
          )
        }) {
          Ok(Some(formatted_text)) => {
            incremental_cache.update_file(&file_path, &formatted_text);
            write_file_contents(&file_path, &formatted_text)?;
            formatted_files_count.fetch_add(1, Ordering::Relaxed);
            let _g = output_lock.lock();
            info!("{}", file_path.to_string_lossy());
          }
          Ok(None) => {
            incremental_cache.update_file(&file_path, &file.text);
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
    #[allow(clippy::print_stdout)]
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

  builder.build()
}

fn get_resolved_json_config(
  options: &FmtOptionsConfig,
) -> dprint_plugin_json::configuration::Configuration {
  let mut builder =
    dprint_plugin_json::configuration::ConfigurationBuilder::new();

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

  builder.build()
}

fn get_resolved_malva_config(
  options: &FmtOptionsConfig,
) -> malva::config::FormatOptions {
  use malva::config::*;

  let layout_options = LayoutOptions {
    print_width: options.line_width.unwrap_or(80) as usize,
    use_tabs: options.use_tabs.unwrap_or_default(),
    indent_width: options.indent_width.unwrap_or(2) as usize,
    line_break: LineBreak::Lf,
  };

  let language_options = LanguageOptions {
    align_comments: true,
    hex_case: HexCase::Lower,
    hex_color_length: None,
    quotes: if let Some(true) = options.single_quote {
      Quotes::PreferSingle
    } else {
      Quotes::PreferDouble
    },
    operator_linebreak: OperatorLineBreak::Before,
    block_selector_linebreak: BlockSelectorLineBreak::Consistent,
    omit_number_leading_zero: false,
    trailing_comma: true,
    format_comments: false,
    linebreak_in_pseudo_parens: true,
    declaration_order: None,
    single_line_block_threshold: None,
    keyframe_selector_notation: None,
    attr_value_quotes: AttrValueQuotes::Always,
    prefer_single_line: false,
    selectors_prefer_single_line: None,
    function_args_prefer_single_line: None,
    sass_content_at_rule_prefer_single_line: None,
    sass_include_at_rule_prefer_single_line: None,
    sass_map_prefer_single_line: None,
    sass_module_config_prefer_single_line: None,
    sass_params_prefer_single_line: None,
    less_import_options_prefer_single_line: None,
    less_mixin_args_prefer_single_line: None,
    less_mixin_params_prefer_single_line: None,
    single_line_top_level_declarations: false,
    selector_override_comment_directive: "deno-fmt-selector-override".into(),
    ignore_comment_directive: "deno-fmt-ignore".into(),
    ignore_file_comment_directive: "deno-fmt-ignore-file".into(),
    declaration_order_group_by: DeclarationOrderGroupBy::NonDeclaration,
  };

  FormatOptions {
    layout: layout_options,
    language: language_options,
  }
}

fn get_resolved_markup_fmt_config(
  options: &FmtOptionsConfig,
) -> markup_fmt::config::FormatOptions {
  use markup_fmt::config::*;

  let layout_options = LayoutOptions {
    print_width: options.line_width.unwrap_or(80) as usize,
    use_tabs: options.use_tabs.unwrap_or_default(),
    indent_width: options.indent_width.unwrap_or(2) as usize,
    line_break: LineBreak::Lf,
  };

  let language_options = LanguageOptions {
    script_formatter: Some(markup_fmt::config::ScriptFormatter::Dprint),
    quotes: Quotes::Double,
    format_comments: false,
    script_indent: true,
    html_script_indent: None,
    vue_script_indent: Some(false),
    svelte_script_indent: None,
    astro_script_indent: None,
    style_indent: true,
    html_style_indent: None,
    vue_style_indent: Some(false),
    svelte_style_indent: None,
    astro_style_indent: None,
    closing_bracket_same_line: false,
    closing_tag_line_break_for_empty: ClosingTagLineBreakForEmpty::Fit,
    max_attrs_per_line: None,
    prefer_attrs_single_line: false,
    single_attr_same_line: false,
    html_normal_self_closing: None,
    html_void_self_closing: None,
    component_self_closing: None,
    svg_self_closing: None,
    mathml_self_closing: None,
    whitespace_sensitivity: WhitespaceSensitivity::Css,
    component_whitespace_sensitivity: None,
    doctype_keyword_case: DoctypeKeywordCase::Upper,
    v_bind_style: None,
    v_on_style: None,
    v_for_delimiter_style: None,
    v_slot_style: None,
    component_v_slot_style: None,
    default_v_slot_style: None,
    named_v_slot_style: None,
    v_bind_same_name_short_hand: None,
    strict_svelte_attr: false,
    svelte_attr_shorthand: Some(true),
    svelte_directive_shorthand: Some(true),
    astro_attr_shorthand: Some(true),
    ignore_comment_directive: "deno-fmt-ignore".into(),
    ignore_file_comment_directive: "deno-fmt-ignore-file".into(),
  };

  FormatOptions {
    layout: layout_options,
    language: language_options,
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
        | "sass"
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
    assert!(is_supported_ext_fmt(Path::new("foo.sass")));
    assert!(is_supported_ext_fmt(Path::new("foo.Sass")));
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
}
