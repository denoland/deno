// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_ast::LineAndColumnIndex;
use deno_ast::ModuleSpecifier;
use deno_ast::SourceTextInfo;
use deno_graph::Module;
use deno_graph::Position;
use deno_graph::Range;
use deno_graph::Resolved;

use super::mappings::Mappings;
use super::specifiers::is_remote_specifier_text;
use super::text_changes::TextChange;

struct Context<'a> {
  mappings: &'a Mappings,
  module: &'a Module,
  text_info: &'a SourceTextInfo,
  text: &'a str,
  text_changes: Vec<TextChange>,
  local_path: PathBuf,
}

impl<'a> Context<'a> {
  pub fn byte_index(&self, pos: &Position) -> usize {
    // todo(https://github.com/denoland/deno_graph/issues/79): use byte indexes all the way down
    self
      .text_info
      .byte_index(LineAndColumnIndex {
        line_index: pos.line,
        column_index: pos.character,
      })
      .0 as usize
  }

  pub fn byte_range(&self, range: &Range) -> std::ops::Range<usize> {
    let start = self.byte_index(&range.start);
    let end = self.byte_index(&range.end);
    start..end
  }

  pub fn relative_specifier_text(&self, specifier: &ModuleSpecifier) -> String {
    let local_path = self.mappings.local_path(specifier);
    get_relative_specifier_text(&self.local_path, local_path)
  }
}

pub fn collect_remote_module_text_changes<'a>(
  mappings: &'a Mappings,
  module: &'a Module,
) -> Vec<TextChange> {
  let text_info = match &module.maybe_parsed_source {
    Some(source) => source.source(),
    None => return Vec::new(),
  };
  let text = match &module.maybe_source {
    Some(source) => source,
    None => return Vec::new(),
  };
  let mut context = Context {
    mappings,
    module,
    text_info,
    text,
    text_changes: Vec::new(),
    local_path: mappings.local_path(&module.specifier).to_owned(),
  };

  // todo(THIS PR): this is may not good enough because it only includes
  // what deno_graph has resolved and may not include everything in the source file?
  for dep in context.module.dependencies.values() {
    handle_maybe_resolved(&dep.maybe_code, &mut context);
    handle_maybe_resolved(&dep.maybe_type, &mut context);
  }

  // todo(THIS PR): does this contain more than just the header? I think so?

  // resolve x-typescript-types header and inject it as a types directive
  if let Some((_, Resolved::Ok { specifier, .. })) =
    &context.module.maybe_types_dependency
  {
    let new_specifier_text = context.relative_specifier_text(specifier);
    context.text_changes.push(TextChange::new(
      0,
      0,
      format!("/// <reference types=\"{}\" />\n", new_specifier_text),
    ))
  }

  context.text_changes
}

fn handle_maybe_resolved(maybe_resolved: &Resolved, context: &mut Context<'_>) {
  if let Resolved::Ok {
    specifier, range, ..
  } = maybe_resolved
  {
    let mut byte_range = context.byte_range(range);
    let mut current_text = &context.text[byte_range.clone()];
    if current_text.starts_with('"') || current_text.starts_with('\'') {
      // remove the quotes
      byte_range = (byte_range.start + 1)..(byte_range.end - 1);
      current_text = &context.text[byte_range.clone()];
    };

    // leave remote specifiers as-is as they will be handled by the import map
    if !is_remote_specifier_text(current_text) {
      let new_specifier_text = context.relative_specifier_text(specifier);
      context.text_changes.push(TextChange::new(
        byte_range.start,
        byte_range.end,
        new_specifier_text,
      ));
    }
  }
}

fn get_relative_specifier_text(from: &Path, to: &Path) -> String {
  let relative_path = get_relative_path(from, to);

  if relative_path.starts_with("../") || relative_path.starts_with("./") {
    relative_path
  } else {
    format!("./{}", relative_path)
  }
}

fn get_relative_path(from: &Path, to: &Path) -> String {
  let from_path = ModuleSpecifier::from_file_path(from).unwrap();
  let to_path = ModuleSpecifier::from_file_path(to).unwrap();
  from_path.make_relative(&to_path).unwrap()
}
