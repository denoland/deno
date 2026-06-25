// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::SourceRangedForSpanned as _;
use deno_ast::swc::ast;
use deno_ast::swc::atoms::Atom;
use deno_ast::swc::common::DUMMY_SP;
use deno_ast::swc::common::comments::CommentKind;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitMut;
use deno_ast::swc::ecma_visit::VisitWith as _;
use deno_ast::swc::ecma_visit::visit_mut_pass;
use deno_ast::swc::utils as swc_utils;
use deno_cache_dir::file_fetcher::File;
use deno_core::ModuleSpecifier;
use deno_core::error::AnyError;
use regex::Regex;

use crate::args::PermissionFlags;
use crate::args::flags_from_vec;
use crate::file_fetcher::TextDecodedFile;
use crate::util::path::mapped_specifier_for_tsc;

/// Extracts doc tests from a given file, transforms them into pseudo test
/// files by wrapping the content of the doc tests in a `Deno.test` call, and
/// returns a list of the pseudo test files.
///
/// The difference from [`extract_snippet_files`] is that this function wraps
/// extracted code snippets in a `Deno.test` call.
pub fn extract_doc_tests(file: File) -> Result<Vec<File>, AnyError> {
  extract_inner(file, WrapKind::DenoTest)
}

/// Extracts code snippets from a given file and returns a list of the extracted
/// files.
///
/// The difference from [`extract_doc_tests`] is that this function does *not*
/// wrap extracted code snippets in a `Deno.test` call.
pub fn extract_snippet_files(file: File) -> Result<Vec<File>, AnyError> {
  extract_inner(file, WrapKind::NoWrap)
}

#[derive(Clone, Copy)]
enum WrapKind {
  DenoTest,
  NoWrap,
}

struct TestOrSnippet {
  file: File,
  has_deno_test: bool,
  shebang: Option<Shebang>,
}

fn extract_inner(
  file: File,
  wrap_kind: WrapKind,
) -> Result<Vec<File>, AnyError> {
  let file = TextDecodedFile::decode(file)?;

  let exports = match deno_ast::parse_program(deno_ast::ParseParams {
    specifier: file.specifier.clone(),
    text: file.source.clone(),
    media_type: file.media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  }) {
    Ok(parsed) => {
      let mut c = ExportCollector::default();
      c.visit_program(parsed.program().as_ref());
      c
    }
    Err(_) => ExportCollector::default(),
  };

  let extracted_files =
    if matches!(file.media_type, MediaType::Unknown | MediaType::Markdown) {
      extract_files_from_fenced_blocks(
        &file.specifier,
        &file.source,
        file.media_type,
      )?
    } else {
      extract_files_from_source_comments(
        &file.specifier,
        file.source.clone(),
        file.media_type,
      )?
    };

  extracted_files
    .into_iter()
    .map(|extracted| {
      let wrap_kind = if extracted.has_deno_test {
        WrapKind::NoWrap
      } else {
        wrap_kind
      };
      generate_pseudo_file(
        extracted.file,
        &file.specifier,
        &exports,
        wrap_kind,
        extracted.shebang.as_ref(),
      )
    })
    .collect::<Result<_, _>>()
}

fn extract_files_from_fenced_blocks(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
) -> Result<Vec<TestOrSnippet>, AnyError> {
  let lines_regex = lazy_regex::regex!(r"(((#!+).*)|(?:# ?)?(.*))");

  Ok(
    extract_markdown_fenced_blocks(source)
      .into_iter()
      .filter_map(|block| {
        let file_media_type =
          media_type_from_fence_attributes(block.attributes, media_type)?;
        extract_file_from_block(
          specifier,
          file_media_type,
          /* file line index */ 0,
          block.line_offset,
          block.line_count,
          block.body,
          block.is_markdown_blockquote,
          lines_regex,
        )
      })
      .collect(),
  )
}

struct MarkdownFence<'a> {
  attributes: &'a str,
  body: &'a str,
  line_offset: usize,
  line_count: usize,
  is_markdown_blockquote: bool,
}

fn extract_markdown_fenced_blocks(source: &str) -> Vec<MarkdownFence<'_>> {
  let mut lines = Vec::new();
  let mut offset = 0;
  for line in source.split_inclusive('\n') {
    let line_start = offset;
    offset += line.len();
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    lines.push((line_start, line));
  }
  if offset < source.len() {
    lines.push((offset, &source[offset..]));
  }

  let mut blocks = Vec::new();
  let mut line_index = 0;
  let mut in_html_comment = false;
  while line_index < lines.len() {
    if in_html_comment {
      if lines[line_index].1.contains("-->") {
        in_html_comment = false;
      }
      line_index += 1;
      continue;
    }
    let line = lines[line_index].1.trim_start_matches([' ', '\t']);
    if line.starts_with("<!--") {
      if !line.contains("-->") {
        in_html_comment = true;
      }
      line_index += 1;
      continue;
    }

    let Some(opening) = parse_markdown_fence_opening(lines[line_index].1)
    else {
      line_index += 1;
      continue;
    };

    let mut closing_line_index = line_index + 1;
    while closing_line_index < lines.len() {
      if is_markdown_fence_closing(
        lines[closing_line_index].1,
        opening.tick_count,
      ) {
        let body_start = lines
          .get(line_index + 1)
          .map(|(offset, _)| *offset)
          .unwrap_or(source.len());
        let body_end = lines[closing_line_index].0;
        blocks.push(MarkdownFence {
          attributes: opening.attributes,
          body: &source[body_start..body_end],
          line_offset: line_index,
          line_count: closing_line_index - line_index + 1,
          is_markdown_blockquote: opening.is_markdown_blockquote,
        });
        line_index = closing_line_index + 1;
        break;
      }
      closing_line_index += 1;
    }

    if closing_line_index == lines.len() {
      line_index += 1;
    }
  }

  blocks
}

struct MarkdownFenceOpening<'a> {
  attributes: &'a str,
  tick_count: usize,
  is_markdown_blockquote: bool,
}

fn parse_markdown_fence_opening(
  line: &str,
) -> Option<MarkdownFenceOpening<'_>> {
  let (line, is_markdown_blockquote) = strip_markdown_fence_prefix(line);
  let tick_count = line.bytes().take_while(|b| *b == b'`').count();
  if tick_count < 3 {
    return None;
  }
  Some(MarkdownFenceOpening {
    attributes: &line[tick_count..],
    tick_count,
    is_markdown_blockquote,
  })
}

fn is_markdown_fence_closing(line: &str, opening_tick_count: usize) -> bool {
  let (line, _) = strip_markdown_fence_prefix(line);
  let tick_count = line.bytes().take_while(|b| *b == b'`').count();
  tick_count >= opening_tick_count
    && line[tick_count..].bytes().all(|b| b == b' ' || b == b'\t')
}

fn strip_markdown_fence_prefix(line: &str) -> (&str, bool) {
  let mut line = line.trim_start_matches([' ', '\t']);
  let mut is_markdown_blockquote = false;
  while let Some(after_marker) = line.strip_prefix('>') {
    is_markdown_blockquote = true;
    line = after_marker
      .strip_prefix([' ', '\t'])
      .unwrap_or(after_marker);
  }
  (line, is_markdown_blockquote)
}

fn extract_files_from_source_comments(
  specifier: &ModuleSpecifier,
  source: Arc<str>,
  media_type: MediaType,
) -> Result<Vec<TestOrSnippet>, AnyError> {
  let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
    specifier: specifier.clone(),
    text: source,
    media_type,
    capture_tokens: false,
    maybe_syntax: None,
    scope_analysis: false,
  })?;
  let comments = parsed_source.comments().get_vec();
  let blocks_regex = lazy_regex::regex!(
    r"```(?P<attributes>[^\r\n]*)\r?\n(?P<body>[\S\s]*?)```"
  );
  let lines_regex =
    lazy_regex::regex!(r"(?:\* ?)((#!+).*)|(?:\* ?)(?:\# ?)?(.*)");

  let files = comments
    .iter()
    .filter(|comment| {
      if comment.kind != CommentKind::Block || !comment.text.starts_with('*') {
        return false;
      }

      true
    })
    .flat_map(|comment| {
      extract_files_from_regex_blocks(
        specifier,
        &comment.text,
        media_type,
        parsed_source.text_info_lazy().line_index(comment.start()),
        blocks_regex,
        lines_regex,
      )
    })
    .flatten()
    .collect();

  Ok(files)
}

fn extract_files_from_regex_blocks(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
  file_line_index: usize,
  blocks_regex: &Regex,
  lines_regex: &Regex,
) -> Result<Vec<TestOrSnippet>, AnyError> {
  let files = blocks_regex
    .captures_iter(source)
    .filter_map(|block| {
      let is_markdown_blockquote = block
        .name("blockquote")
        .is_some_and(|blockquote| !blockquote.as_str().is_empty());
      block.name("attributes")?;

      let maybe_attributes: Option<Vec<_>> = block
        .name("attributes")
        .map(|attributes| attributes.as_str().split(' ').collect());

      let file_media_type =
        media_type_from_attributes(maybe_attributes, media_type)?;

      if file_media_type == MediaType::Unknown {
        return None;
      }

      let line_offset = source[0..block.get(0).unwrap().start()]
        .chars()
        .filter(|c| *c == '\n')
        .count();

      let line_count = block.get(0).unwrap().as_str().split('\n').count();

      let body = block.name("body").unwrap();
      extract_file_from_block(
        specifier,
        file_media_type,
        file_line_index,
        line_offset,
        line_count,
        body.as_str(),
        is_markdown_blockquote,
        lines_regex,
      )
    })
    .collect();

  Ok(files)
}

fn media_type_from_fence_attributes(
  attributes: &str,
  fallback_media_type: MediaType,
) -> Option<MediaType> {
  media_type_from_attributes(
    Some(attributes.split(' ').collect()),
    fallback_media_type,
  )
}

fn media_type_from_attributes(
  maybe_attributes: Option<Vec<&str>>,
  fallback_media_type: MediaType,
) -> Option<MediaType> {
  let Some(attributes) = maybe_attributes else {
    return Some(fallback_media_type);
  };
  if attributes.contains(&"ignore") {
    return None;
  }

  Some(match attributes.first() {
    Some(&"js") => MediaType::JavaScript,
    Some(&"javascript") => MediaType::JavaScript,
    Some(&"mjs") => MediaType::Mjs,
    Some(&"cjs") => MediaType::Cjs,
    Some(&"jsx") => MediaType::Jsx,
    Some(&"ts") => MediaType::TypeScript,
    Some(&"typescript") => MediaType::TypeScript,
    Some(&"mts") => MediaType::Mts,
    Some(&"cts") => MediaType::Cts,
    Some(&"tsx") => MediaType::Tsx,
    _ => MediaType::Unknown,
  })
}

#[allow(
  clippy::too_many_arguments,
  reason = "Keeps source location metadata explicit for both markdown and JSDoc extraction callers."
)]
fn extract_file_from_block(
  specifier: &ModuleSpecifier,
  file_media_type: MediaType,
  file_line_index: usize,
  line_offset: usize,
  line_count: usize,
  text: &str,
  is_markdown_blockquote: bool,
  lines_regex: &Regex,
) -> Option<TestOrSnippet> {
  let tests_regex = lazy_regex::regex!(r"(?m)^\s*Deno\.test\(");

  if file_media_type == MediaType::Unknown {
    return None;
  }

  // TODO(caspervonb) generate an inline source map
  let mut file_source = String::new();
  let mut shebang = None;
  let mut is_first_line = true;
  for line in text.lines() {
    let line = if is_markdown_blockquote {
      strip_markdown_blockquote_marker(line)
    } else {
      line
    };
    let Some(line) = lines_regex.captures(line) else {
      continue;
    };
    let text = line.get(1).or_else(|| line.get(3)).unwrap().as_str();
    // Strip shebang from the very first line to forward it to `Deno.test`.
    if is_first_line && text.starts_with("#!") {
      shebang = Some(parse_shebang(text));
      is_first_line = false;
      continue;
    }
    is_first_line = false;
    writeln!(file_source, "{}", text).unwrap();
  }

  let file_specifier = ModuleSpecifier::parse(&format!(
    "{}#{}-{}",
    specifier,
    file_line_index + line_offset + 1,
    file_line_index + line_offset + line_count + 1,
  ))
  .unwrap();
  let file_specifier =
    mapped_specifier_for_tsc(&file_specifier, file_media_type)
      .map(|s| ModuleSpecifier::parse(&s).unwrap())
      .unwrap_or_else(|| {
        // The tsc mapping only appends an extension when the path's media
        // type differs; do it here too so every virtual file keeps one.
        ModuleSpecifier::parse(&format!(
          "{}{}",
          file_specifier,
          file_media_type.as_ts_extension()
        ))
        .unwrap()
      });
  let has_deno_test = tests_regex.is_match(&file_source);
  let file = File {
    url: file_specifier,
    mtime: None,
    // The fragment (line range + extension) is ignored when inferring the
    // media type from the path, so carry it via a content-type header.
    maybe_headers: file_media_type.as_content_type().map(|content_type| {
      HashMap::from([("content-type".to_string(), content_type.to_string())])
    }),
    source: file_source.into_bytes().into(),
    loaded_from: deno_cache_dir::file_fetcher::LoadedFrom::Local,
  };
  Some(TestOrSnippet {
    file,
    has_deno_test,
    shebang,
  })
}

fn strip_markdown_blockquote_marker(line: &str) -> &str {
  let mut spaces = 0;

  for (index, ch) in line.char_indices() {
    match ch {
      ' ' if spaces < 3 => {
        spaces += 1;
      }
      '>' => {
        let after_marker = index + ch.len_utf8();
        return line[after_marker..]
          .strip_prefix([' ', '\t'])
          .unwrap_or(&line[after_marker..]);
      }
      _ => break,
    }
  }

  line
}

enum Shebang {
  Permissions(Box<PermissionFlags>),
  /// Message describing why the shebang could not be parsed as a `deno`
  /// command. The generated test is made to throw with this message.
  Invalid(String),
}

/// Parses a shebang line like `#!/usr/bin/env -S deno run --allow-read`.
///
/// The flags are parsed using deno's own CLI argument parser and the resulting
/// permissions are forwarded to `Deno.test`.
///
/// Known limitations:
/// - The `deno` executable is matched by file name, so custom-named binaries
///   (e.g. `deno-canary`) are not recognized and yield an invalid shebang.
/// - A scoped `--deny-*=<path>` cannot be represented in the `Deno.test`
///   permissions object yet and yields an invalid shebang to force failure
/// - A `--ignore-*` cannnot be represented in the `Deno.test` permissions
///   object either yet, so they are currently ignored
fn parse_shebang(shebang: &str) -> Shebang {
  let invalid = |reason: &str| {
    Shebang::Invalid(format!(
      "invalid doc test hashbang: {} ({reason})",
      shebang.trim()
    ))
  };
  let Some(line) = shebang.trim_start().strip_prefix("#!") else {
    return invalid("invalid hashbang");
  };
  let Some(tokens) = shlex::split(line) else {
    return invalid("tokenization failed, possibly due to unterminated quotes");
  };
  // Find the `deno` executable in the shebang (e.g. `deno`, `/usr/bin/deno`).
  let Some(deno_index) = tokens.iter().position(|token| {
    std::path::Path::new(token)
      .file_stem()
      .and_then(|stem| stem.to_str())
      == Some("deno")
  }) else {
    return invalid("binary basename needs to be 'deno'");
  };
  let mut args = vec![OsString::from("deno")];
  args.extend(tokens[deno_index + 1..].iter().cloned().map(OsString::from));
  args.push(OsString::from("./__doctest_shebang__.ts"));
  match flags_from_vec(args) {
    Ok(flags) if has_scoped_deny(&flags.permissions) => invalid(
      "scoped --deny-* flags aren't supported yet, either remove them or ignore the test",
    ),
    Ok(flags) => Shebang::Permissions(Box::new(flags.permissions)),
    Err(err) => match err.get(clap::error::ContextKind::InvalidArg) {
      Some(clap::error::ContextValue::String(arg)) => {
        invalid(&format!("could not parse flag {arg}"))
      }
      Some(clap::error::ContextValue::Strings(args)) => {
        invalid(&format!("could not parse flags {}", args.join(", ")))
      }
      _ => invalid("could not parse flags"),
    },
  }
}

fn has_scoped_deny(permissions: &PermissionFlags) -> bool {
  [
    &permissions.deny_env,
    &permissions.deny_ffi,
    &permissions.deny_import,
    &permissions.deny_net,
    &permissions.deny_read,
    &permissions.deny_run,
    &permissions.deny_sys,
    &permissions.deny_write,
  ]
  .into_iter()
  .any(|deny| matches!(deny, Some(list) if !list.is_empty()))
}

#[derive(Default)]
struct ExportCollector {
  named_exports: BTreeSet<Atom>,
  /// Subset of `named_exports` whose declaration was a TypeScript-only
  /// construct (`type` alias, `interface`, or an explicit `export type {}`
  /// re-export). When generating injected imports for doc tests we emit
  /// these with the per-specifier `type` modifier so projects with
  /// `verbatimModuleSyntax: true` don't fail type-checking
  /// (denoland/deno#31385).
  named_type_only_exports: BTreeSet<Atom>,
  default_export: Option<Atom>,
}

impl ExportCollector {
  /// Record `name` as a value export. A value export always wins over a
  /// type-only export of the same identifier (e.g. a `const` and an
  /// `interface` sharing a name via declaration merging), so this clears any
  /// prior type-only marking to keep the injected import a value import.
  fn add_value_export(&mut self, name: Atom) {
    self.named_type_only_exports.remove(&name);
    self.named_exports.insert(name);
  }

  /// Record `name` as a type-only export, unless it is already known to be a
  /// value export (in which case the value import must be preserved).
  fn add_type_only_export(&mut self, name: Atom) {
    let already_value = self.named_exports.contains(&name)
      && !self.named_type_only_exports.contains(&name);
    if !already_value {
      self.named_type_only_exports.insert(name.clone());
    }
    self.named_exports.insert(name);
  }

  fn to_import_specifiers(
    &self,
    symbols_to_exclude: &rustc_hash::FxHashSet<Atom>,
    symbols_to_include: &rustc_hash::FxHashSet<Atom>,
  ) -> Vec<ast::ImportSpecifier> {
    let mut import_specifiers = vec![];

    if let Some(default_export) = &self.default_export {
      // If the default export conflicts with a named export, a named one
      // takes precedence.
      if !symbols_to_exclude.contains(default_export)
        && symbols_to_include.contains(default_export)
        && !self.named_exports.contains(default_export)
      {
        import_specifiers.push(ast::ImportSpecifier::Default(
          ast::ImportDefaultSpecifier {
            span: DUMMY_SP,
            local: ast::Ident {
              span: DUMMY_SP,
              ctxt: Default::default(),
              sym: default_export.clone(),
              optional: false,
            },
          },
        ));
      }
    }

    for named_export in &self.named_exports {
      if symbols_to_exclude.contains(named_export)
        || !symbols_to_include.contains(named_export)
      {
        continue;
      }
      if !is_importable_binding_identifier(named_export) {
        continue;
      }

      import_specifiers.push(ast::ImportSpecifier::Named(
        ast::ImportNamedSpecifier {
          span: DUMMY_SP,
          local: ast::Ident {
            span: DUMMY_SP,
            ctxt: Default::default(),
            sym: named_export.clone(),
            optional: false,
          },
          imported: None,
          is_type_only: self.named_type_only_exports.contains(named_export),
        },
      ));
    }

    import_specifiers
  }
}

#[derive(Default)]
struct UnresolvedIdentCollector {
  unresolved_context: deno_ast::swc::common::SyntaxContext,
  atoms: rustc_hash::FxHashSet<Atom>,
}

impl Visit for UnresolvedIdentCollector {
  fn visit_ident(&mut self, ident: &ast::Ident) {
    if ident.ctxt == self.unresolved_context {
      self.atoms.insert(ident.sym.clone());
    }
  }
}

fn is_importable_binding_identifier(name: &Atom) -> bool {
  swc_utils::is_valid_ident(name.as_ref())
}

impl Visit for ExportCollector {
  fn visit_ts_module_decl(&mut self, ts_module_decl: &ast::TsModuleDecl) {
    if ts_module_decl.declare {
      return;
    }

    ts_module_decl.visit_children_with(self);
  }

  fn visit_export_decl(&mut self, export_decl: &ast::ExportDecl) {
    match &export_decl.decl {
      ast::Decl::Class(class) => {
        self.add_value_export(class.ident.sym.clone());
      }
      ast::Decl::Fn(func) => {
        self.add_value_export(func.ident.sym.clone());
      }
      ast::Decl::Var(var) => {
        for var_decl in &var.decls {
          for atom in extract_sym_from_pat(&var_decl.name) {
            self.add_value_export(atom);
          }
        }
      }
      ast::Decl::TsEnum(ts_enum) => {
        self.add_value_export(ts_enum.id.sym.clone());
      }
      ast::Decl::TsModule(ts_module) => {
        if ts_module.declare {
          return;
        }

        match &ts_module.id {
          ast::TsModuleName::Ident(ident) => {
            self.add_value_export(ident.sym.clone());
          }
          ast::TsModuleName::Str(s) => {
            self.add_value_export(s.value.to_atom_lossy().into_owned());
          }
        }
      }
      ast::Decl::TsTypeAlias(ts_type_alias) => {
        self.add_type_only_export(ts_type_alias.id.sym.clone());
      }
      ast::Decl::TsInterface(ts_interface) => {
        self.add_type_only_export(ts_interface.id.sym.clone());
      }
      ast::Decl::Using(_) => {}
    }
  }

  fn visit_export_default_decl(
    &mut self,
    export_default_decl: &ast::ExportDefaultDecl,
  ) {
    match &export_default_decl.decl {
      ast::DefaultDecl::Class(class) => {
        if let Some(ident) = &class.ident {
          self.default_export = Some(ident.sym.clone());
        }
      }
      ast::DefaultDecl::Fn(func) => {
        if let Some(ident) = &func.ident {
          self.default_export = Some(ident.sym.clone());
        }
      }
      ast::DefaultDecl::TsInterfaceDecl(iface_decl) => {
        self.default_export = Some(iface_decl.id.sym.clone());
      }
    }
  }

  fn visit_export_default_expr(
    &mut self,
    export_default_expr: &ast::ExportDefaultExpr,
  ) {
    if let ast::Expr::Ident(ident) = &*export_default_expr.expr {
      self.default_export = Some(ident.sym.clone());
    }
  }

  fn visit_named_export(&mut self, named_export: &ast::NamedExport) {
    fn get_atom(export_name: &ast::ModuleExportName) -> Atom {
      match export_name {
        ast::ModuleExportName::Ident(ident) => ident.sym.clone(),
        ast::ModuleExportName::Str(s) => s.value.to_atom_lossy().into_owned(),
      }
    }

    // For re-exports of the form `export { foo } from "./other.ts"` the names
    // listed in the specifiers are still part of *this* module's export
    // surface, so doc-test snippets should be able to use them without an
    // explicit import (denoland/deno#30550). Namespace re-exports are
    // `ExportSpecifier::Namespace` and skipped below.
    let is_reexport = named_export.src.is_some();

    for specifier in &named_export.specifiers {
      let ast::ExportSpecifier::Named(named) = specifier else {
        continue;
      };
      let name = match &named.exported {
        Some(exported) => get_atom(exported),
        None => get_atom(&named.orig),
      };
      // `export { default } from "./other.ts"` re-exports the default through
      // this module — there's no new *named* export surface to inject.
      if is_reexport && name.as_ref() == "default" {
        continue;
      }
      // `export type { Foo }` (`named_export.type_only`) and
      // `export { type Foo }` (`named.is_type_only`) both make the binding
      // type-only.
      let is_type_only = named_export.type_only || named.is_type_only;
      if is_type_only {
        self.add_type_only_export(name);
      } else {
        self.add_value_export(name);
      }
    }
  }
}

fn extract_sym_from_pat(pat: &ast::Pat) -> Vec<Atom> {
  fn rec(pat: &ast::Pat, atoms: &mut Vec<Atom>) {
    match pat {
      ast::Pat::Ident(binding_ident) => {
        atoms.push(binding_ident.sym.clone());
      }
      ast::Pat::Array(array_pat) => {
        for elem in array_pat.elems.iter().flatten() {
          rec(elem, atoms);
        }
      }
      ast::Pat::Rest(rest_pat) => {
        rec(&rest_pat.arg, atoms);
      }
      ast::Pat::Object(object_pat) => {
        for prop in &object_pat.props {
          match prop {
            ast::ObjectPatProp::Assign(assign_pat_prop) => {
              atoms.push(assign_pat_prop.key.sym.clone());
            }
            ast::ObjectPatProp::KeyValue(key_value_pat_prop) => {
              rec(&key_value_pat_prop.value, atoms);
            }
            ast::ObjectPatProp::Rest(rest_pat) => {
              rec(&rest_pat.arg, atoms);
            }
          }
        }
      }
      ast::Pat::Assign(assign_pat) => {
        rec(&assign_pat.left, atoms);
      }
      ast::Pat::Invalid(_) | ast::Pat::Expr(_) => {}
    }
  }

  let mut atoms = vec![];
  rec(pat, &mut atoms);
  atoms
}

/// Generates a "pseudo" file from a given file by applying the following
/// transformations:
///
/// 1. Injects `import` statements for expoted items from the base file
/// 2. If `wrap_kind` is [`WrapKind::DenoTest`], wraps the content of the file
///    in a `Deno.test` call.
///
/// For example, given a file that looks like:
///
/// ```ts
/// import { assertEquals } from "@std/assert/equals";
///
/// assertEquals(increment(1), 2);
/// ```
///
/// and the base file (from which the above snippet was extracted):
///
/// ```ts
/// export function increment(n: number): number {
///   return n + 1;
/// }
///
/// export const SOME_CONST = "HELLO";
/// ```
///
/// The generated pseudo test file would look like (if `wrap_in_deno_test` is enabled):
///
/// ```ts
/// import { assertEquals } from "@std/assert/equals";
/// import { increment, SOME_CONST } from "./base.ts";
///
/// Deno.test("./base.ts#1-3.ts", async () => {
///   assertEquals(increment(1), 2);
/// });
/// ```
///
/// # Edge case 1 - duplicate identifier
///
/// If a given file imports, say, `doSomething` from an external module while
/// the base file exports `doSomething` as well, the generated pseudo test file
/// would end up having two duplciate imports for `doSomething`, causing the
/// duplicate identifier error.
///
/// To avoid this issue, when a given file imports `doSomething`, this takes
/// precedence over the automatic import injection for the base file's
/// `doSomething`. So the generated pseudo test file would look like:
///
/// ```ts
/// import { assertEquals } from "@std/assert/equals";
/// import { doSomething } from "./some_external_module.ts";
///
/// Deno.test("./base.ts#1-3.ts", async () => {
///   assertEquals(doSomething(1), 2);
/// });
/// ```
///
/// # Edge case 2 - exports can't be put inside `Deno.test` blocks
///
/// All exports like `export const foo = 42` must be at the top level of the
/// module, making it impossible to wrap exports in `Deno.test` blocks. For
/// example, when the following code snippet is provided:
///
/// ```ts
/// const logger = createLogger("my-awesome-module");
///
/// export function sum(a: number, b: number): number {
///   logger.debug("sum called");
///   return a + b;
/// }
/// ```
///
/// If we applied the naive transformation to this, the generated pseudo test
/// file would look like:
///
/// ```ts
/// Deno.test("./base.ts#1-7.ts", async () => {
///   const logger = createLogger("my-awesome-module");
///
///   export function sum(a: number, b: number): number {
///     logger.debug("sum called");
///     return a + b;
///   }
/// });
/// ```
///
/// But obviously this violates the rule because `export function sum` is not
/// at the top level of the module.
///
/// To address this issue, the `export` keyword is removed so that the item can
/// stay in the `Deno.test` block's scope:
///
/// ```ts
/// Deno.test("./base.ts#1-7.ts", async () => {
///   const logger = createLogger("my-awesome-module");
///
///   function sum(a: number, b: number): number {
///     logger.debug("sum called");
///     return a + b;
///   }
/// });
/// ```
fn generate_pseudo_file(
  file: File,
  base_file_specifier: &ModuleSpecifier,
  exports: &ExportCollector,
  wrap_kind: WrapKind,
  shebang: Option<&Shebang>,
) -> Result<File, AnyError> {
  let file = TextDecodedFile::decode(file)?;

  let parsed = deno_ast::parse_program(deno_ast::ParseParams {
    specifier: file.specifier.clone(),
    text: file.source,
    media_type: file.media_type,
    capture_tokens: false,
    scope_analysis: true,
    maybe_syntax: None,
  })?;

  let top_level_atoms = swc_utils::collect_decls_with_ctxt::<Atom, _>(
    &parsed.program_ref(),
    parsed.top_level_context(),
  );
  let mut unresolved_ident_collector = UnresolvedIdentCollector {
    unresolved_context: parsed.unresolved_context(),
    ..Default::default()
  };
  parsed
    .program_ref()
    .visit_with(&mut unresolved_ident_collector);

  let transformed =
    parsed
      .program_ref()
      .to_owned()
      .apply(&mut visit_mut_pass(Transform {
        specifier: &file.specifier,
        base_file_specifier,
        exports_from_base: exports,
        atoms_to_be_excluded_from_import: top_level_atoms,
        atoms_to_be_included_from_import: unresolved_ident_collector.atoms,
        wrap_kind,
        shebang,
      }));

  let source = deno_ast::swc::codegen::to_code_with_comments(
    Some(&parsed.comments().as_single_threaded()),
    &transformed,
  );

  log::debug!("{}:\n{}", file.specifier, source);

  Ok(File {
    url: file.specifier,
    mtime: None,
    // The fragment (line range + extension) is ignored when inferring the
    // media type from the path, so carry it via a content-type header.
    maybe_headers: file.media_type.as_content_type().map(|content_type| {
      HashMap::from([("content-type".to_string(), content_type.to_string())])
    }),
    source: source.into_bytes().into(),
    loaded_from: deno_cache_dir::file_fetcher::LoadedFrom::Local,
  })
}

struct Transform<'a> {
  specifier: &'a ModuleSpecifier,
  base_file_specifier: &'a ModuleSpecifier,
  exports_from_base: &'a ExportCollector,
  atoms_to_be_excluded_from_import: rustc_hash::FxHashSet<Atom>,
  atoms_to_be_included_from_import: rustc_hash::FxHashSet<Atom>,
  wrap_kind: WrapKind,
  shebang: Option<&'a Shebang>,
}

impl VisitMut for Transform<'_> {
  fn visit_mut_program(&mut self, node: &mut ast::Program) {
    let new_module_items = match node {
      ast::Program::Module(module) => {
        let mut module_decls = vec![];
        let mut stmts = vec![];

        for item in &module.body {
          match item {
            ast::ModuleItem::ModuleDecl(decl) => match self.wrap_kind {
              WrapKind::NoWrap => {
                module_decls.push(decl.clone());
              }
              // We remove `export` keywords so that they can be put inside
              // `Deno.test` block scope.
              WrapKind::DenoTest => match decl {
                ast::ModuleDecl::ExportDecl(export_decl) => {
                  stmts.push(ast::Stmt::Decl(export_decl.decl.clone()));
                }
                ast::ModuleDecl::ExportDefaultDecl(export_default_decl) => {
                  let stmt = match &export_default_decl.decl {
                    ast::DefaultDecl::Class(class) => {
                      let expr = ast::Expr::Class(class.clone());
                      ast::Stmt::Expr(ast::ExprStmt {
                        span: DUMMY_SP,
                        expr: Box::new(expr),
                      })
                    }
                    ast::DefaultDecl::Fn(func) => {
                      let expr = ast::Expr::Fn(func.clone());
                      ast::Stmt::Expr(ast::ExprStmt {
                        span: DUMMY_SP,
                        expr: Box::new(expr),
                      })
                    }
                    ast::DefaultDecl::TsInterfaceDecl(ts_interface_decl) => {
                      ast::Stmt::Decl(ast::Decl::TsInterface(
                        ts_interface_decl.clone(),
                      ))
                    }
                  };
                  stmts.push(stmt);
                }
                ast::ModuleDecl::ExportDefaultExpr(export_default_expr) => {
                  stmts.push(ast::Stmt::Expr(ast::ExprStmt {
                    span: DUMMY_SP,
                    expr: export_default_expr.expr.clone(),
                  }));
                }
                _ => {
                  module_decls.push(decl.clone());
                }
              },
            },
            ast::ModuleItem::Stmt(stmt) => {
              stmts.push(stmt.clone());
            }
          }
        }

        let mut transformed_items = vec![];
        transformed_items
          .extend(module_decls.into_iter().map(ast::ModuleItem::ModuleDecl));
        let import_specifiers = self.exports_from_base.to_import_specifiers(
          &self.atoms_to_be_excluded_from_import,
          &self.atoms_to_be_included_from_import,
        );
        if !import_specifiers.is_empty() {
          transformed_items.push(ast::ModuleItem::ModuleDecl(
            ast::ModuleDecl::Import(ast::ImportDecl {
              span: DUMMY_SP,
              specifiers: import_specifiers,
              src: Box::new(ast::Str {
                span: DUMMY_SP,
                value: self.base_file_specifier.to_string().into(),
                raw: None,
              }),
              type_only: false,
              with: None,
              phase: ast::ImportPhase::Evaluation,
            }),
          ));
        }
        match self.wrap_kind {
          WrapKind::DenoTest => {
            transformed_items.push(ast::ModuleItem::Stmt(wrap_in_deno_test(
              stmts,
              self.specifier.to_string().into(),
              self.shebang,
            )));
          }
          WrapKind::NoWrap => {
            transformed_items
              .extend(stmts.into_iter().map(ast::ModuleItem::Stmt));
          }
        }

        transformed_items
      }
      ast::Program::Script(script) => {
        let mut transformed_items = vec![];

        let import_specifiers = self.exports_from_base.to_import_specifiers(
          &self.atoms_to_be_excluded_from_import,
          &self.atoms_to_be_included_from_import,
        );
        if !import_specifiers.is_empty() {
          transformed_items.push(ast::ModuleItem::ModuleDecl(
            ast::ModuleDecl::Import(ast::ImportDecl {
              span: DUMMY_SP,
              specifiers: import_specifiers,
              src: Box::new(ast::Str {
                span: DUMMY_SP,
                value: self.base_file_specifier.to_string().into(),
                raw: None,
              }),
              type_only: false,
              with: None,
              phase: ast::ImportPhase::Evaluation,
            }),
          ));
        }

        match self.wrap_kind {
          WrapKind::DenoTest => {
            transformed_items.push(ast::ModuleItem::Stmt(wrap_in_deno_test(
              script.body.clone(),
              self.specifier.to_string().into(),
              self.shebang,
            )));
          }
          WrapKind::NoWrap => {
            transformed_items.extend(
              script.body.clone().into_iter().map(ast::ModuleItem::Stmt),
            );
          }
        }

        transformed_items
      }
    };

    *node = ast::Program::Module(ast::Module {
      span: DUMMY_SP,
      body: new_module_items,
      shebang: None,
    });
  }
}

fn wrap_in_deno_test(
  mut stmts: Vec<ast::Stmt>,
  test_name: Atom,
  shebang: Option<&Shebang>,
) -> ast::Stmt {
  // Forward permissions declared by a shebang (if present).
  // Mark the test as invalid if specified command is unparseable.
  let options = match shebang {
    Some(Shebang::Permissions(permissions)) => {
      Some(permissions_options_object(permissions))
    }
    Some(Shebang::Invalid(message)) => {
      stmts.insert(
        0,
        ast::Stmt::Throw(ast::ThrowStmt {
          span: DUMMY_SP,
          arg: Box::new(ast::Expr::New(ast::NewExpr {
            callee: Box::new(ast::Expr::Ident(ast::Ident {
              span: DUMMY_SP,
              sym: "Error".into(),
              optional: false,
              ..Default::default()
            })),
            args: Some(vec![ast::ExprOrSpread {
              spread: None,
              expr: Box::new(string_lit(message)),
            }]),
            ..Default::default()
          })),
        }),
      );
      None
    }
    None => None,
  };

  ast::Stmt::Expr(ast::ExprStmt {
    span: DUMMY_SP,
    expr: Box::new(ast::Expr::Call(ast::CallExpr {
      span: DUMMY_SP,
      callee: ast::Callee::Expr(Box::new(ast::Expr::Member(ast::MemberExpr {
        span: DUMMY_SP,
        obj: Box::new(ast::Expr::Ident(ast::Ident {
          span: DUMMY_SP,
          sym: "Deno".into(),
          optional: false,
          ..Default::default()
        })),
        prop: ast::MemberProp::Ident(ast::IdentName {
          span: DUMMY_SP,
          sym: "test".into(),
        }),
      }))),
      args: [
        Some(string_lit(test_name.as_str())),
        options,
        Some(ast::Expr::Arrow(ast::ArrowExpr {
          span: DUMMY_SP,
          params: vec![],
          body: Box::new(ast::BlockStmtOrExpr::BlockStmt(ast::BlockStmt {
            span: DUMMY_SP,
            stmts,
            ..Default::default()
          })),
          is_async: true,
          is_generator: false,
          type_params: None,
          return_type: None,
          ..Default::default()
        })),
      ]
      .into_iter()
      .flatten()
      .map(|expr| ast::ExprOrSpread {
        spread: None,
        expr: Box::new(expr),
      })
      .collect(),
      type_args: None,
      ..Default::default()
    })),
  })
}

fn string_lit(value: &str) -> ast::Expr {
  ast::Expr::Lit(ast::Lit::Str(ast::Str {
    span: DUMMY_SP,
    value: value.into(),
    raw: None,
  }))
}

/// Builds the `{ permissions: ... }` options object passed to `Deno.test` for a
/// snippet that declared a shebang.
///
/// - `--allow-all` becomes `{ permissions: "inherit" }`.
/// - `--allow-*` becomes `{ [*]: "inherit" }`.
/// - `--allow-*=...` becomes ` { [*]: [...] }`.
/// - `--deny-*` becomes `{ [*]: false }`.
/// - `--deny-*=...` is currently unsupported
/// - `--permission-set` is currently unsupported
/// - `--ignore-*` is currently unsupported
/// - No permissions flags becomes `{ permissions: "none" }`
///
/// The currently unsupported flags are due to the current
/// `Deno.PermissionOptionsObject` not being able to properly model them.
fn permissions_options_object(permissions: &PermissionFlags) -> ast::Expr {
  let prop = |name: &str, value: ast::Expr| {
    ast::PropOrSpread::Prop(Box::new(ast::Prop::KeyValue(ast::KeyValueProp {
      key: ast::PropName::Ident(ast::IdentName {
        span: DUMMY_SP,
        sym: name.into(),
      }),
      value: Box::new(value),
    })))
  };
  let bool_lit = |value: bool| {
    ast::Expr::Lit(ast::Lit::Bool(ast::Bool {
      span: DUMMY_SP,
      value,
    }))
  };

  let perms = [
    ("env", &permissions.allow_env, &permissions.deny_env),
    ("ffi", &permissions.allow_ffi, &permissions.deny_ffi),
    (
      "import",
      &permissions.allow_import,
      &permissions.deny_import,
    ),
    ("net", &permissions.allow_net, &permissions.deny_net),
    ("read", &permissions.allow_read, &permissions.deny_read),
    ("run", &permissions.allow_run, &permissions.deny_run),
    ("sys", &permissions.allow_sys, &permissions.deny_sys),
    ("write", &permissions.allow_write, &permissions.deny_write),
  ];

  let value = if permissions.allow_all
    && perms.iter().all(|(_, _, deny)| deny.is_none())
  {
    string_lit("inherit")
  } else {
    let props = perms
      .into_iter()
      .filter_map(|(name, allow, deny)| {
        let value = if deny.is_some() {
          bool_lit(false)
        } else if permissions.allow_all {
          string_lit("inherit")
        } else if let Some(allowlist) = allow {
          if allowlist.is_empty() {
            string_lit("inherit")
          } else {
            ast::Expr::Array(ast::ArrayLit {
              span: DUMMY_SP,
              elems: allowlist
                .iter()
                .map(|entry| {
                  Some(ast::ExprOrSpread {
                    spread: None,
                    expr: Box::new(string_lit(entry)),
                  })
                })
                .collect(),
            })
          }
        } else {
          return None;
        };
        Some(prop(name, value))
      })
      .collect::<Vec<_>>();

    if props.is_empty() {
      string_lit("none")
    } else {
      ast::Expr::Object(ast::ObjectLit {
        span: DUMMY_SP,
        props,
      })
    }
  };

  ast::Expr::Object(ast::ObjectLit {
    span: DUMMY_SP,
    props: vec![prop("permissions", value)],
  })
}

#[cfg(test)]
mod tests {
  use deno_ast::swc::atoms::Atom;
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::file_fetcher::TextDecodedFile;

  #[test]
  fn test_extract_doc_tests() {
    struct Input {
      source: &'static str,
      specifier: &'static str,
    }
    struct Expected {
      source: &'static str,
      specifier: &'static str,
      media_type: MediaType,
    }
    struct Test {
      input: Input,
      expected: Vec<Expected>,
    }

    let tests = [
      Test {
        input: Input {
          source: r#""#,
          specifier: "file:///main.ts",
        },
        expected: vec![],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { assertEquals } from "@std/assert/equal";
 *
 * assertEquals(add(1, 2), 3);
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equal";
import { add } from "file:///main.ts";
Deno.test("file:///main.ts#3-8.ts", async ()=>{
    assertEquals(add(1, 2), 3);
});
"#,
          specifier: "file:///main.ts#3-8.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * foo();
 * ```
 */
export function foo() {}

export default class Bar {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-6.ts", async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * const input = { a: 42 } satisfies Args;
 * foo(input);
 * ```
 */
export function foo(args: Args) {}

export type Args = { a: number };
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { type Args, foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", async ()=>{
    const input = {
        a: 42
    } satisfies Args;
    foo(input);
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * Documentation of my function.
 *
 * @example Usage
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read
 * foo("bar")
 * ```
 */
export function foo(s: string) {
  return s;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#6-10.ts", {
    permissions: {
        read: "inherit"
    }
}, async ()=>{
    foo("bar");
});
"#,
          specifier: "file:///main.ts#6-10.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * This is a module-level doc.
 *
 * ```ts
 * foo();
 * ```
 *
 * @module doc
 */
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"Deno.test("file:///main.ts#5-8.ts", async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#5-8.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * This is a module-level doc.
 *
 * ```js
 * const cls = new MyClass();
 * ```
 *
 * @module doc
 */

/**
 * ```ts
 * foo();
 * ```
 */
export function foo() {}

export default class MyClass {}

export * from "./other.ts";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![
          Expected {
            source: r#"import MyClass from "file:///main.ts";
Deno.test("file:///main.ts#5-8.js", async ()=>{
    const cls = new MyClass();
});
"#,
            specifier: "file:///main.ts#5-8.js",
            media_type: MediaType::JavaScript,
          },
          Expected {
            source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#13-16.ts", async ()=>{
    foo();
});
"#,
            specifier: "file:///main.ts#13-16.ts",
            media_type: MediaType::TypeScript,
          },
        ],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * foo();
 * ```
 */
export function foo() {}

export const ONE = 1;
const TWO = 2;
export default TWO;
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-6.ts", async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Avoid duplicate imports
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { DUPLICATE1 } from "./other1.ts";
 * import * as DUPLICATE2 from "./other2.js";
 * import { foo as DUPLICATE3 } from "./other3.tsx";
 *
 * foo();
 * ```
 */
export function foo() {}

export const DUPLICATE1 = "dup1";
const DUPLICATE2 = "dup2";
export default DUPLICATE2;
const DUPLICATE3 = "dup3";
export { DUPLICATE3 };
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { DUPLICATE1 } from "./other1.ts";
import * as DUPLICATE2 from "./other2.js";
import { foo as DUPLICATE3 } from "./other3.tsx";
import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-10.ts", async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-10.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // duplication of imported identifier and local identifier is fine
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * const foo = createFoo();
 * foo();
 * ```
 */
export function createFoo() {
  return () => "created foo";
}

export const foo = () => "foo";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { createFoo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", async ()=>{
    const foo = createFoo();
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/25718
      // A case where the example code has an exported item which references
      // a variable from one upper scope.
      // Naive application of `Deno.test` wrap would cause a reference error
      // because the variable would go inside the `Deno.test` block while the
      // exported item would be moved to the top level. To suppress the auto
      // move of the exported item to the top level, the `export` keyword is
      // removed so that the item stays in the same scope as the variable.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { getLogger } from "@std/log";
 *
 * const logger = getLogger("my-awesome-module");
 *
 * export function foo() {
 *   logger.debug("hello");
 * }
 * ```
 *
 * @module
 */
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { getLogger } from "@std/log";
Deno.test("file:///main.ts#3-12.ts", async ()=>{
    const logger = getLogger("my-awesome-module");
    function foo() {
        logger.debug("hello");
    }
});
"#,
          specifier: "file:///main.ts#3-12.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
# Header

This is a *markdown*.

```js
import { assertEquals } from "@std/assert/equal";
import { add } from "jsr:@deno/non-existent";

assertEquals(add(1, 2), 3);
```
"#,
          specifier: "file:///README.md",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equal";
import { add } from "jsr:@deno/non-existent";
Deno.test("file:///README.md#6-12.js", async ()=>{
    assertEquals(add(1, 2), 3);
});
"#,
          specifier: "file:///README.md#6-12.js",
          media_type: MediaType::JavaScript,
        }],
      },
      // https://github.com/denoland/deno/issues/11640
      Test {
        input: Input {
          source: r#"
````
```ts
````


````
```ts
##
````
"#,
          specifier: "file:///README.md",
        },
        expected: vec![],
      },
      Test {
        input: Input {
          source: r#"
# Header

````ts
console.log("ts");
````
"#,
          specifier: "file:///README.md",
        },
        expected: vec![Expected {
          source: r#"Deno.test("file:///README.md#4-7.ts", async ()=>{
    console.log("ts");
});
"#,
          specifier: "file:///README.md#4-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/24164
      Test {
        input: Input {
          source: r#"
# Header

> ```ts
> import { assertEquals } from "@std/assert/equals";
>
> assertEquals(1 + 2, 3);
> ```
"#,
          specifier: "file:///README.md",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equals";
Deno.test("file:///README.md#4-9.ts", async ()=>{
    assertEquals(1 + 2, 3);
});
"#,
          specifier: "file:///README.md#4-9.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26009
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * console.log(Foo)
 * ```
 */
export class Foo {}
export default Foo
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { Foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-6.ts", async ()=>{
    console.log(Foo);
});
"#,
          specifier: "file:///main.ts#3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26728
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * // @ts-expect-error: can only add numbers
 * add('1', '2');
 * ```
 */
export function add(first: number, second: number) {
  return first + second;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { add } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", async ()=>{
    // @ts-expect-error: can only add numbers
    add('1', '2');
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26900
      // Do not inject exports that the snippet does not reference. Otherwise,
      // projects with `noUnusedLocals` fail on imports generated by Deno.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { randomNonce } from "@test/csp/value";
 *
 * randomNonce();
 * ```
 */
export function formatNonceValue(n: string): string {
  return n;
}

export function randomNonce(): string {
  return crypto.randomUUID();
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { randomNonce } from "@test/csp/value";
Deno.test("file:///main.ts#3-8.ts", async ()=>{
    randomNonce();
});
"#,
          specifier: "file:///main.ts#3-8.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/29629
      Test {
        input: Input {
          source: r#"
# Title

```ts
import { assertEquals } from "@std/assert/equals";

Deno.test("add", () => {
  assertEquals(1 + 2, 3);
});
```
"#,
          specifier: "file:///main.md",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equals";
Deno.test("add", ()=>{
    assertEquals(1 + 2, 3);
});
"#,
          specifier: "file:///main.md#4-11.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 *
 * Deno.test("add", () => {
 *   assertEquals(add(1, 2), 3);
 * });
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}
  "#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equals";
import { add } from "file:///main.ts";
Deno.test("add", ()=>{
    assertEquals(add(1, 2), 3);
});
"#,
          specifier: "file:///main.ts#3-10.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // commented out `Deno.test` should be ignored
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 * // Deno.test("add", () => {});
 * assertEquals(add(1, 2), 3);
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}
  "#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equals";
import { add } from "file:///main.ts";
Deno.test("file:///main.ts#3-8.ts", async ()=>{
    // Deno.test("add", () => {});
    assertEquals(add(1, 2), 3);
});
"#,
          specifier: "file:///main.ts#3-8.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Shebang with `--allow-all` inherits everything.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * #!/usr/bin/env -S deno run -A
 * foo();
 * ```
 */
export function foo() {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", {
    permissions: "inherit"
}, async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Shebang without any permission flag runs with no permissions.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * #!/usr/bin/env -S deno run
 * foo();
 * ```
 */
export function foo() {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", {
    permissions: "none"
}, async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Shebang with bare permissions flags inherits.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read
 * foo();
 * ```
 */
export function foo() {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", {
    permissions: {
        read: "inherit"
    }
}, async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Shebang with scoped permissions flag forwards the allow-list.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read=/tmp,/var
 * foo();
 * ```
 */
export function foo() {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", {
    permissions: {
        read: [
            "/tmp",
            "/var"
        ]
    }
}, async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Unparseable shebang fails the test, naming the reason.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * #!/bin/sh
 * foo();
 * ```
 */
export function foo() {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", async ()=>{
    throw new Error("invalid doc test hashbang: #!/bin/sh (binary basename needs to be 'deno')");
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // A scoped `--deny-*` can't be modelled, so the test is made to fail
      // rather than run with broader permissions than the shebang declares.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read --deny-read=/etc
 * foo();
 * ```
 */
export function foo() {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", async ()=>{
    throw new Error("invalid doc test hashbang: #!/usr/bin/env -S deno run --allow-read --deny-read=/etc (scoped --deny-* flags aren't supported yet, either remove them or ignore the test)");
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // A bare `--deny-*` becomes `false`; only the flags present in the
      // shebang end up in the permissions object.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read --deny-env
 * foo();
 * ```
 */
export function foo() {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", {
    permissions: {
        env: false,
        read: "inherit"
    }
}, async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // A quoted allow-list path (with a space) is tokenized by `shlex`.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read="/tmp/with space"
 * foo();
 * ```
 */
export function foo() {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { foo } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", {
    permissions: {
        read: [
            "/tmp/with space"
        ]
    }
}, async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // A markdown code block (not a JSDoc comment) also supports shebangs.
      Test {
        input: Input {
          source: r#"# Title

```ts
#!/usr/bin/env -S deno run --allow-net=example.com
foo();
```
"#,
          specifier: "file:///README.md",
        },
        expected: vec![Expected {
          source: r#"Deno.test("file:///README.md#3-7.ts", {
    permissions: {
        net: [
            "example.com"
        ]
    }
}, async ()=>{
    foo();
});
"#,
          specifier: "file:///README.md#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Regression test for https://github.com/denoland/deno/issues/31385
      // Type-only exports must be injected with the per-specifier `type`
      // modifier so doc tests don't fail under `verbatimModuleSyntax: true`.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * const foo: Foo = "foo";
 * const bar: Bar = { x: 1 };
 * new Quux();
 * useFoo();
 * ```
 */
export function useFoo() {}
export type Foo = string;
export interface Bar { x: number }
export class Quux {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { type Bar, type Foo, Quux, useFoo } from "file:///main.ts";
Deno.test("file:///main.ts#3-9.ts", async ()=>{
    const foo: Foo = "foo";
    const bar: Bar = {
        x: 1
    };
    new Quux();
    useFoo();
});
"#,
          specifier: "file:///main.ts#3-9.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Regression test for https://github.com/denoland/deno/issues/35177
      // Export names can be reserved words or string-literal names that cannot
      // be used as local import bindings in the generated doc-test module.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * useFoo();
 * ```
 */
export function useFoo() {}
const null_ = null;
const dashed = 1;
export { null_ as null, dashed as "key-with-hyphens" };
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { useFoo } from "file:///main.ts";
Deno.test("file:///main.ts#3-6.ts", async ()=>{
    useFoo();
});
"#,
          specifier: "file:///main.ts#3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * useFoo();
 * ```
 */
export function useFoo() {}
export { nullValue as null, dashed as "key-with-hyphens" } from "./deps.ts";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { useFoo } from "file:///main.ts";
Deno.test("file:///main.ts#3-6.ts", async ()=>{
    useFoo();
});
"#,
          specifier: "file:///main.ts#3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // A name that is both a value export and a type export via declaration
      // merging (here `const Foo` + `interface Foo`) must be injected as a
      // value import, otherwise the value binding is dropped under
      // `verbatimModuleSyntax: true`. The order of the value/type
      // declarations must not matter.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * console.log(Foo);
 * doSomething();
 * ```
 */
export function doSomething() {}
export interface Foo { x: number }
export const Foo = 1;
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { Foo, doSomething } from "file:///main.ts";
Deno.test("file:///main.ts#3-7.ts", async ()=>{
    console.log(Foo);
    doSomething();
});
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
    ];

    for test in tests {
      let file = File {
        url: ModuleSpecifier::parse(test.input.specifier).unwrap(),
        maybe_headers: None,
        mtime: None,
        source: test.input.source.as_bytes().into(),
        loaded_from: deno_cache_dir::file_fetcher::LoadedFrom::Local,
      };
      let got_decoded = extract_doc_tests(file)
        .unwrap()
        .into_iter()
        .map(|f| TextDecodedFile::decode(f).unwrap())
        .collect::<Vec<_>>();
      let expected = test
        .expected
        .iter()
        .map(|e| TextDecodedFile {
          specifier: ModuleSpecifier::parse(e.specifier).unwrap(),
          media_type: e.media_type,
          source: e.source.into(),
        })
        .collect::<Vec<_>>();
      assert_eq!(got_decoded, expected);
    }
  }

  #[test]
  fn test_extract_snippet_files() {
    struct Input {
      source: &'static str,
      specifier: &'static str,
    }
    struct Expected {
      source: &'static str,
      specifier: &'static str,
      media_type: MediaType,
    }
    struct Test {
      input: Input,
      expected: Vec<Expected>,
    }

    let tests = [
      Test {
        input: Input {
          source: r#""#,
          specifier: "file:///main.ts",
        },
        expected: vec![],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 *
 * assertEquals(add(1, 2), 3);
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equals";
import { add } from "file:///main.ts";
assertEquals(add(1, 2), 3);
"#,
          specifier: "file:///main.ts#3-8.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 * import { DUPLICATE } from "./other.ts";
 *
 * assertEquals(add(1, 2), 3);
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}

export const DUPLICATE = "dup";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equals";
import { DUPLICATE } from "./other.ts";
import { add } from "file:///main.ts";
assertEquals(add(1, 2), 3);
"#,
          specifier: "file:///main.ts#3-9.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // If the snippet has a local variable with the same name as an exported
      // item, the local variable takes precedence.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * const foo = createFoo();
 * foo();
 * ```
 */
export function createFoo() {
  return () => "created foo";
}

export const foo = () => "foo";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { createFoo } from "file:///main.ts";
const foo = createFoo();
foo();
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Unlike `extract_doc_tests`, `extract_snippet_files` does not remove
      // the `export` keyword from the exported items.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { getLogger } from "@std/log";
 *
 * const logger = getLogger("my-awesome-module");
 *
 * export function foo() {
 *   logger.debug("hello");
 * }
 * ```
 *
 * @module
 */
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { getLogger } from "@std/log";
export function foo() {
    logger.debug("hello");
}
const logger = getLogger("my-awesome-module");
"#,
          specifier: "file:///main.ts#3-12.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
# Header

This is a *markdown*.

```js
import { assertEquals } from "@std/assert/equal";
import { add } from "jsr:@deno/non-existent";

assertEquals(add(1, 2), 3);
```
"#,
          specifier: "file:///README.md",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equal";
import { add } from "jsr:@deno/non-existent";
assertEquals(add(1, 2), 3);
"#,
          specifier: "file:///README.md#6-12.js",
          media_type: MediaType::JavaScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26009
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * console.log(Foo)
 * ```
 */
export class Foo {}
export default Foo
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { Foo } from "file:///main.ts";
console.log(Foo);
"#,
          specifier: "file:///main.ts#3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26728
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * // @ts-expect-error: can only add numbers
 * add('1', '2');
 * ```
 */
export function add(first: number, second: number) {
  return first + second;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { add } from "file:///main.ts";
// @ts-expect-error: can only add numbers
add('1', '2');
"#,
          specifier: "file:///main.ts#3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
    ];

    for test in tests {
      let file = File {
        url: ModuleSpecifier::parse(test.input.specifier).unwrap(),
        maybe_headers: None,
        mtime: None,
        source: test.input.source.as_bytes().into(),
        loaded_from: deno_cache_dir::file_fetcher::LoadedFrom::Local,
      };
      let got_decoded = extract_snippet_files(file)
        .unwrap()
        .into_iter()
        .map(|f| TextDecodedFile::decode(f).unwrap())
        .collect::<Vec<_>>();
      let expected = test
        .expected
        .iter()
        .map(|e| TextDecodedFile {
          specifier: ModuleSpecifier::parse(e.specifier).unwrap(),
          media_type: e.media_type,
          source: e.source.into(),
        })
        .collect::<Vec<_>>();
      assert_eq!(got_decoded, expected);
    }
  }

  #[test]
  fn test_is_importable_binding_identifier() {
    assert!(is_importable_binding_identifier(&"foo".into()));
    assert!(is_importable_binding_identifier(&"$foo".into()));
    assert!(is_importable_binding_identifier(&"_foo".into()));
    assert!(is_importable_binding_identifier(&"async".into()));
    assert!(!is_importable_binding_identifier(&"null".into()));
    assert!(!is_importable_binding_identifier(&"default".into()));
    assert!(!is_importable_binding_identifier(
      &"key-with-hyphens".into()
    ));
    assert!(!is_importable_binding_identifier(&"with spaces".into()));
    assert!(!is_importable_binding_identifier(&"".into()));
  }

  #[test]
  fn test_export_collector() {
    fn helper(input: &'static str) -> ExportCollector {
      let mut collector = ExportCollector::default();
      let parsed = deno_ast::parse_module(deno_ast::ParseParams {
        specifier: deno_ast::ModuleSpecifier::parse("file:///main.ts").unwrap(),
        text: input.into(),
        media_type: deno_ast::MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
      })
      .unwrap();

      parsed.program_ref().visit_with(&mut collector);
      collector
    }

    struct Test {
      input: &'static str,
      named_expected: BTreeSet<Atom>,
      default_expected: Option<Atom>,
    }

    macro_rules! atom_set {
      ($( $x:expr ),*) => {
        [$( Atom::from($x) ),*].into_iter().collect::<BTreeSet<_>>()
      };
    }

    let tests = [
      Test {
        input: r#"export const foo = 42;"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export let foo = 42;"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export var foo = 42;"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export const foo = () => {};"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export function foo() {}"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export class Foo {}"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export enum Foo {}"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export module Foo {}"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export module "foo" {}"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export namespace Foo {}"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export type Foo = string;"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export interface Foo {};"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export let name1, name2;"#,
        named_expected: atom_set!("name1", "name2"),
        default_expected: None,
      },
      Test {
        input: r#"export const name1 = 1, name2 = 2;"#,
        named_expected: atom_set!("name1", "name2"),
        default_expected: None,
      },
      Test {
        input: r#"export function* generatorFunc() {}"#,
        named_expected: atom_set!("generatorFunc"),
        default_expected: None,
      },
      Test {
        input: r#"export const { name1, name2: bar } = obj;"#,
        named_expected: atom_set!("name1", "bar"),
        default_expected: None,
      },
      Test {
        input: r#"export const [name1, name2] = arr;"#,
        named_expected: atom_set!("name1", "name2"),
        default_expected: None,
      },
      Test {
        input: r#"export const { name1 = 42 } = arr;"#,
        named_expected: atom_set!("name1"),
        default_expected: None,
      },
      Test {
        input: r#"export default function foo() {}"#,
        named_expected: atom_set!(),
        default_expected: Some("foo".into()),
      },
      Test {
        input: r#"export default class Foo {}"#,
        named_expected: atom_set!(),
        default_expected: Some("Foo".into()),
      },
      Test {
        input: r#"export default interface Foo {}"#,
        named_expected: atom_set!(),
        default_expected: Some("Foo".into()),
      },
      Test {
        input: r#"const foo = 42; export default foo;"#,
        named_expected: atom_set!(),
        default_expected: Some("foo".into()),
      },
      Test {
        input: r#"export { foo, bar as barAlias };"#,
        named_expected: atom_set!("foo", "barAlias"),
        default_expected: None,
      },
      Test {
        input: r#"export { foo as null, bar as "key-with-hyphens" };"#,
        named_expected: atom_set!("null", "key-with-hyphens"),
        default_expected: None,
      },
      Test {
        input: r#"
export default class Foo {}
export let value1 = 42;
const value2 = "Hello";
const value3 = "World";
export { value2 };
"#,
        named_expected: atom_set!("value1", "value2"),
        default_expected: Some("Foo".into()),
      },
      // overloaded function
      Test {
        input: r#"
export function foo(a: number): boolean;
export function foo(a: boolean): string;
export function foo(a: number | boolean): boolean | string {
  return typeof a === "number" ? true : "hello";
}
"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      // Re-exports of the form `export { name } from "./other.ts"` are
      // collected so doc-test snippets that reference them resolve
      // (denoland/deno#30550). Namespace re-exports (`export *`,
      // `export * as ns`) and `export { default }` re-exports are still
      // skipped because we'd have to follow the re-export chain to know
      // their actual exported names.
      Test {
        input: r#"
export * from "./module1.ts";
export * as name1 from "./module2.ts";
export { name2, name3 as N3 } from "./module3.js";
export { default } from "./module4.ts";
export { default as myDefault } from "./module5.ts";
"#,
        named_expected: atom_set!("name2", "N3", "myDefault"),
        default_expected: None,
      },
      Test {
        input: r#"
export namespace Foo {
  export type MyType = string;
  export const myValue = 42;
  export function myFunc(): boolean;
}
"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"
declare namespace Foo {
  export type MyType = string;
  export const myValue = 42;
  export function myFunc(): boolean;
}
"#,
        named_expected: atom_set!(),
        default_expected: None,
      },
      Test {
        input: r#"
declare module Foo {
  export type MyType = string;
  export const myValue = 42;
  export function myFunc(): boolean;
}
"#,
        named_expected: atom_set!(),
        default_expected: None,
      },
      Test {
        input: r#"
declare global {
  export type MyType = string;
  export const myValue = 42;
  export function myFunc(): boolean;
}
"#,
        named_expected: atom_set!(),
        default_expected: None,
      },
      // The identifier `Foo` conflicts, but `ExportCollector` doesn't do
      // anything about it. It is handled by `to_import_specifiers` method.
      Test {
        input: r#"
export class Foo {}
export default Foo
"#,
        named_expected: atom_set!("Foo"),
        default_expected: Some("Foo".into()),
      },
    ];

    for test in tests {
      let got = helper(test.input);
      assert_eq!(got.named_exports, test.named_expected);
      assert_eq!(got.default_export, test.default_expected);
    }
  }
}
