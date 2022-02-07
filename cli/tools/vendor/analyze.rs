// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_ast::LineAndColumnIndex;
use deno_ast::ModuleSpecifier;
use deno_ast::SourceTextInfo;
use deno_ast::swc::common::BytePos;
use deno_ast::swc::common::Span;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::Position;
use deno_graph::Range;
use deno_graph::Resolved;

use super::mappings::Mappings;
use super::text_changes::TextChange;

pub struct CollectSpecifierTextChangesParams<'a> {
  pub mappings: &'a Mappings,
  pub module: &'a Module,
  pub graph: &'a ModuleGraph,
}

struct Context<'a> {
  mappings: &'a Mappings,
  module: &'a Module,
  graph: &'a ModuleGraph,
  text_info: &'a SourceTextInfo,
  text_changes: Vec<TextChange>,
  local_path: PathBuf,
}

impl<'a> Context<'a> {
  pub fn byte_pos(&self, pos: &Position) -> BytePos {
    // todo(https://github.com/denoland/deno_graph/issues/79): use byte indexes all the way down
    self.text_info.byte_index(LineAndColumnIndex {
        line_index: pos.line,
        column_index: pos.character,
    })
  }

  pub fn span(&self, range: &Range) -> Span {
    let start = self.byte_pos(&range.start);
    let end = self.byte_pos(&range.end);
    Span::new(start, end, Default::default())
  }
}

pub fn collect_specifier_text_changes(params: &CollectSpecifierTextChangesParams) -> Vec<TextChange> {
  let text_info = match &params.module.maybe_parsed_source {
    Some(source) => source.source(),
    None => return Vec::new(),
  };
  let mut context = Context {
    mappings: params.mappings,
    module: params.module,
    graph: params.graph,
    text_info,
    text_changes: Vec::new(),
    local_path: params.mappings.local_path(&params.module.specifier).to_owned(),
  };

  // todo(dsherret): this is may not good enough because it only includes what deno_graph has resolved
  // and may not include everything in the source file
  for (specifier, dep) in &params.module.dependencies {
    handle_maybe_resolved(&dep.maybe_code, &mut context);
    handle_maybe_resolved(&dep.maybe_type, &mut context);
  }

  context.text_changes
}

fn handle_maybe_resolved(maybe_resolved: &Resolved, context: &mut Context<'_>) {
  if let Resolved::Ok { specifier, range, .. } = maybe_resolved {
    let span = context.span(range);
    let local_path = context.mappings.local_path(specifier);
    let new_specifier = get_relative_specifier(&context.local_path, &local_path);
    context.text_changes.push(TextChange::from_span_and_text(
      Span::new(span.lo + BytePos(1), span.hi - BytePos(1), Default::default()),
      new_specifier,
    ));
  }
}

fn get_relative_specifier(
  from: &Path,
  to: &Path,
) -> String {
  let relative_path = get_relative_path(from, to);

  if relative_path.starts_with("../") || relative_path.starts_with("./")
  {
    relative_path
  } else {
    format!("./{}", relative_path)
  }
}

pub fn get_relative_path(
  from: &Path,
  to: &Path,
) -> String {
  let from_path = ModuleSpecifier::from_file_path(from).unwrap();
  let to_path = ModuleSpecifier::from_file_path(to).unwrap();
  from_path.make_relative(&to_path).unwrap()
}
