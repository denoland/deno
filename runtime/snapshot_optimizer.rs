// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::OnceLock;

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_ast::swc::ast::*;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::swc::ecma_visit::noop_visit_type;
use deno_core::ModuleCodeString;
use sourcemap::SourceMapBuilder;

const NODE_ERRORS_SPECIFIER: &str = "ext:deno_node/internal/errors.ts";
const NODE_ERROR_HELPER_NAME: &str = "makeNodeErrorByCode";
const NODE_ERROR_DEFINE_NAME: &str = "defineNodeError";
const SOURCE_MAP_DIR_ENV: &str = "DENO_SNAPSHOT_OPTIMIZER_SOURCE_MAP_DIR";

pub fn maybe_optimize_source(
  specifier: &str,
  source: ModuleCodeString,
) -> ModuleCodeString {
  let mut tracker = source_map_output_dir()
    .is_some()
    .then(|| RewriteTracker::new(source.as_ref()));
  let mut source =
    maybe_lower_duplicate_namespace(specifier, source, tracker.as_mut());
  source =
    maybe_lower_runtime_global_tables(specifier, source, tracker.as_mut());
  source =
    maybe_lower_lazy_descriptor_selectors(specifier, source, tracker.as_mut());
  source = maybe_lower_node_errors(specifier, source, tracker.as_mut());
  if let Some(tracker) = &tracker
    && let Some(source_map) = tracker.to_source_map(specifier, source.as_ref())
  {
    write_build_source_map("optimizer", specifier, &source_map);
  }
  source
}

#[derive(Clone)]
struct LoweredNodeError {
  name: String,
  base: String,
  start: usize,
  end: usize,
  params: Vec<String>,
  message_prelude: String,
  message_expression: String,
  message_postlude: String,
}

#[derive(Clone)]
struct Replacement {
  range: Range<usize>,
  text: String,
}

#[derive(Clone)]
struct RewriteSpan {
  range: Range<usize>,
  original_start: Option<usize>,
}

struct RewriteTracker {
  original_source: String,
  spans: Vec<RewriteSpan>,
  changed: bool,
}

impl RewriteTracker {
  fn new(original_source: &str) -> Self {
    Self {
      original_source: original_source.to_string(),
      spans: vec![RewriteSpan {
        range: 0..original_source.len(),
        original_start: Some(0),
      }],
      changed: false,
    }
  }

  fn apply_replacements(
    &mut self,
    source_len: usize,
    replacements: &[Replacement],
  ) {
    if replacements.is_empty() {
      return;
    }
    self.changed = true;
    let mut new_spans = Vec::with_capacity(self.spans.len());
    let mut old_cursor = 0;
    let mut new_cursor = 0;
    for replacement in replacements {
      self.push_copied_span(
        old_cursor..replacement.range.start,
        new_cursor,
        &mut new_spans,
      );
      new_cursor += replacement.range.start.saturating_sub(old_cursor);
      let replacement_end = new_cursor + replacement.text.len();
      if !replacement.text.is_empty() {
        new_spans.push(RewriteSpan {
          range: new_cursor..replacement_end,
          original_start: None,
        });
      }
      old_cursor = replacement.range.end;
      new_cursor = replacement_end;
    }
    self.push_copied_span(old_cursor..source_len, new_cursor, &mut new_spans);
    self.spans = new_spans;
  }

  fn push_copied_span(
    &self,
    old_range: Range<usize>,
    new_start: usize,
    new_spans: &mut Vec<RewriteSpan>,
  ) {
    if old_range.is_empty() {
      return;
    }
    for span in &self.spans {
      let start = old_range.start.max(span.range.start);
      let end = old_range.end.min(span.range.end);
      if start >= end {
        continue;
      }
      let offset = start - old_range.start;
      let original_start = span
        .original_start
        .map(|original_start| original_start + start - span.range.start);
      new_spans.push(RewriteSpan {
        range: new_start + offset..new_start + offset + end - start,
        original_start,
      });
    }
  }

  fn to_source_map(
    &self,
    specifier: &str,
    generated_source: &str,
  ) -> Option<Vec<u8>> {
    if !self.changed {
      return None;
    }
    let generated_index = LineIndex::new(generated_source);
    let original_index = LineIndex::new(&self.original_source);
    let mut builder = SourceMapBuilder::new(Some(specifier));
    let source_id = builder.add_source(specifier);
    builder.set_source_contents(source_id, Some(&self.original_source));

    for span in &self.spans {
      if span.range.is_empty() {
        continue;
      }
      for generated_offset in generated_index.line_starts_in(span.range.clone())
      {
        let (dst_line, dst_col) =
          generated_index.line_and_col(generated_offset);
        if let Some(original_start) = span.original_start {
          let original_offset =
            original_start + generated_offset - span.range.start;
          let (src_line, src_col) =
            original_index.line_and_col(original_offset);
          builder.add_raw(
            dst_line,
            dst_col,
            src_line,
            src_col,
            Some(source_id),
            None,
            true,
          );
        } else {
          builder.add_raw(dst_line, dst_col, 0, 0, None, None, false);
        }
      }
    }

    let mut source_map = Vec::new();
    builder.into_sourcemap().to_writer(&mut source_map).ok()?;
    Some(source_map)
  }
}

struct LineIndex {
  line_starts: Vec<usize>,
}

impl LineIndex {
  fn new(source: &str) -> Self {
    let mut line_starts = vec![0];
    for (index, byte) in source.bytes().enumerate() {
      if byte == b'\n' && index + 1 < source.len() {
        line_starts.push(index + 1);
      }
    }
    Self { line_starts }
  }

  fn line_and_col(&self, offset: usize) -> (u32, u32) {
    let line = match self.line_starts.binary_search(&offset) {
      Ok(line) => line,
      Err(line) => line.saturating_sub(1),
    };
    (line as u32, (offset - self.line_starts[line]) as u32)
  }

  fn line_starts_in(&self, range: Range<usize>) -> Vec<usize> {
    let mut offsets = vec![range.start];
    for line_start in &self.line_starts {
      if *line_start > range.start && *line_start < range.end {
        offsets.push(*line_start);
      }
    }
    offsets
  }
}

fn maybe_lower_duplicate_namespace(
  specifier: &str,
  source: ModuleCodeString,
  tracker: Option<&mut RewriteTracker>,
) -> ModuleCodeString {
  if !specifier.starts_with("ext:deno_node/") && !specifier.starts_with("node:")
  {
    return source;
  }
  if !source.contains("default:") {
    return source;
  }
  let source_text = source.to_string();
  let Some(parsed) = parse_snapshot_source(specifier, &source_text) else {
    return source;
  };
  let Some(replacement) =
    duplicate_namespace_replacement(&parsed, &source_text)
  else {
    return source;
  };
  let mut replacements = vec![replacement];
  if let Some(edit) = ensure_object_create_edit(&parsed, &source_text) {
    replacements.push(edit);
  } else {
    return source;
  }
  apply_replacements(&source_text, replacements, tracker).into()
}

fn duplicate_namespace_replacement(
  parsed: &deno_ast::ParsedSource,
  source: &str,
) -> Option<Replacement> {
  struct Collector<'a> {
    text_info: &'a SourceTextInfo,
    source: &'a str,
    default_object_keys: HashMap<String, HashSet<String>>,
    replacement: Option<Replacement>,
  }

  impl Visit for Collector<'_> {
    noop_visit_type!();

    fn visit_return_stmt(&mut self, node: &ReturnStmt) {
      if self.replacement.is_some() {
        return;
      }
      let Some(argument) = &node.arg else {
        return;
      };
      let Expr::Object(object) = &**argument else {
        return;
      };
      let Some(default_binding) = return_default_binding(object) else {
        return;
      };
      if !return_namespace_matches_default_object(
        object,
        &default_binding,
        &self.default_object_keys,
      ) {
        return;
      }
      let range = byte_range(self.text_info, node.range());
      let indent = indentation_at(self.source, range.start);
      self.replacement = Some(Replacement {
        range,
        text: format!(
          "const namespaceExport = ObjectCreate({default_binding});\n{indent}namespaceExport.default = {default_binding};\n{indent}return namespaceExport;"
        ),
      });
    }
  }

  let text_info = parsed.text_info_lazy();
  let mut collector = Collector {
    text_info,
    source,
    default_object_keys: collect_object_literal_keys(parsed),
    replacement: None,
  };
  parsed.program_ref().visit_with(&mut collector);
  collector.replacement
}

fn collect_object_literal_keys(
  parsed: &deno_ast::ParsedSource,
) -> HashMap<String, HashSet<String>> {
  struct Collector {
    objects: HashMap<String, HashSet<String>>,
  }

  impl Visit for Collector {
    noop_visit_type!();

    fn visit_var_declarator(&mut self, node: &VarDeclarator) {
      let Pat::Ident(ident) = &node.name else {
        return;
      };
      let Some(init) = &node.init else {
        return;
      };
      let Expr::Object(object) = &**init else {
        return;
      };
      let mut keys = HashSet::new();
      for prop in &object.props {
        let PropOrSpread::Prop(prop) = prop else {
          continue;
        };
        match &**prop {
          Prop::Shorthand(ident) => {
            keys.insert(ident.sym.to_string());
          }
          Prop::KeyValue(key_value) => {
            if let Some(name) = prop_name(&key_value.key)
              && name != "__proto__"
            {
              keys.insert(name);
            }
          }
          _ => {}
        }
      }
      self.objects.insert(ident.id.sym.to_string(), keys);
    }
  }

  let mut collector = Collector {
    objects: HashMap::new(),
  };
  parsed.program_ref().visit_with(&mut collector);
  collector.objects
}

fn return_default_binding(object: &ObjectLit) -> Option<String> {
  for prop in &object.props {
    let PropOrSpread::Prop(prop) = prop else {
      continue;
    };
    let Prop::KeyValue(key_value) = &**prop else {
      continue;
    };
    if prop_name(&key_value.key).as_deref() == Some("default")
      && let Expr::Ident(ident) = &*key_value.value
    {
      return Some(ident.sym.to_string());
    }
  }
  None
}

fn return_namespace_matches_default_object(
  object: &ObjectLit,
  default_binding: &str,
  default_object_keys: &HashMap<String, HashSet<String>>,
) -> bool {
  let Some(keys) = default_object_keys.get(default_binding) else {
    return false;
  };
  let mut named_exports = 0;
  for prop in &object.props {
    match prop {
      PropOrSpread::Spread(spread) => {
        if !matches!(&*spread.expr, Expr::Ident(ident) if ident.sym == *default_binding)
        {
          return false;
        }
        named_exports += keys.len();
      }
      PropOrSpread::Prop(prop) => match &**prop {
        Prop::KeyValue(key_value)
          if prop_name(&key_value.key).as_deref() == Some("default") => {}
        Prop::Shorthand(ident) => {
          if !keys.contains(&ident.sym.to_string()) {
            return false;
          }
          named_exports += 1;
        }
        Prop::KeyValue(key_value) => {
          let Some(name) = prop_name(&key_value.key) else {
            return false;
          };
          if !keys.contains(&name) {
            return false;
          }
          named_exports += 1;
        }
        _ => return false,
      },
    }
  }
  named_exports > 0
}

fn ensure_object_create_edit(
  parsed: &deno_ast::ParsedSource,
  source: &str,
) -> Option<Replacement> {
  if source.contains("ObjectCreate") {
    return Some(Replacement {
      range: 0..0,
      text: String::new(),
    });
  }
  struct Collector<'a> {
    text_info: &'a SourceTextInfo,
    edit: Option<Replacement>,
  }

  impl Visit for Collector<'_> {
    noop_visit_type!();

    fn visit_var_declarator(&mut self, node: &VarDeclarator) {
      if self.edit.is_some() {
        return;
      }
      if !matches!(&node.init, Some(init) if matches!(&**init, Expr::Ident(ident) if ident.sym == "primordials"))
      {
        return;
      }
      let Pat::Object(object) = &node.name else {
        return;
      };
      for prop in &object.props {
        let ObjectPatProp::KeyValue(key_value) = prop else {
          continue;
        };
        let PropName::Ident(key) = &key_value.key else {
          continue;
        };
        if key.sym == "ObjectCreate" {
          self.edit = Some(Replacement {
            range: 0..0,
            text: String::new(),
          });
          return;
        }
      }
      let Some(first) = object.props.first() else {
        return;
      };
      let range = byte_range(self.text_info, first.range());
      self.edit = Some(Replacement {
        range: range.start..range.start,
        text: "ObjectCreate,\n  ".to_string(),
      });
    }
  }

  let text_info = parsed.text_info_lazy();
  let mut collector = Collector {
    text_info,
    edit: None,
  };
  parsed.program_ref().visit_with(&mut collector);
  if collector.edit.is_some() {
    return collector.edit;
  }
  if let Some(index) = source.find("const { core } = __bootstrap;") {
    return Some(Replacement {
      range: index..index + "const { core } = __bootstrap;".len(),
      text:
        "const { core, primordials } = __bootstrap;\nconst { ObjectCreate } = primordials;"
      .to_string(),
    });
  }
  None
}

fn indentation_at(source: &str, offset: usize) -> &str {
  let mut line_start = offset;
  while line_start > 0 && source.as_bytes()[line_start - 1] != b'\n' {
    line_start -= 1;
  }
  let mut cursor = line_start;
  while cursor < source.len() && source.as_bytes()[cursor] == b' ' {
    cursor += 1;
  }
  &source[line_start..cursor]
}

fn push_literal_replacement(
  source: &str,
  replacements: &mut Vec<Replacement>,
  old: &str,
  new: &str,
) {
  for (start, _) in source.match_indices(old) {
    replacements.push(Replacement {
      range: start..start + old.len(),
      text: new.to_string(),
    });
  }
}

fn maybe_lower_runtime_global_tables(
  specifier: &str,
  source: ModuleCodeString,
  tracker: Option<&mut RewriteTracker>,
) -> ModuleCodeString {
  let lower = match specifier {
    "ext:deno_node/console_esm.ts" | "node:console" => {
      lower_node_console_esm(source.as_ref(), tracker)
    }
    "ext:runtime/90_deno_ns.js" => {
      lower_deno_ns_tables(source.as_ref(), tracker)
    }
    "ext:runtime/98_global_scope_shared.js" => {
      lower_window_or_worker_global_scope_table(source.as_ref(), tracker)
    }
    "ext:runtime/98_global_scope_window.js" => {
      lower_main_runtime_global_table(source.as_ref(), tracker)
    }
    "ext:runtime/98_global_scope_worker.js" => {
      lower_worker_runtime_global_table(source.as_ref(), tracker)
    }
    "ext:runtime/99_main.js" | "ext:runtime_main/js/99_main.js" => {
      lower_runtime_main_global_table_uses(source.as_ref(), tracker)
    }
    _ => return source,
  };
  let Some(lower) = lower else {
    return source;
  };
  lower.into()
}

fn lower_node_console_esm(
  source: &str,
  tracker: Option<&mut RewriteTracker>,
) -> Option<String> {
  let mut replacements = Vec::new();
  push_literal_replacement(
    source,
    &mut replacements,
    "import { windowOrWorkerGlobalScope } from \"ext:runtime/98_global_scope_shared.js\";",
    "import { globalScopeConsole } from \"ext:runtime/98_global_scope_shared.js\";",
  );
  push_literal_replacement(
    source,
    &mut replacements,
    "const console = windowOrWorkerGlobalScope.console.value;",
    "const console = globalScopeConsole;",
  );
  push_literal_replacement(
    source,
    &mut replacements,
    "const _consoleDesc = windowOrWorkerGlobalScope.console as any;\nconst console = typeof _consoleDesc.get === \"function\"\n  ? _consoleDesc.get()\n  : _consoleDesc.value;",
    "const console = globalScopeConsole;",
  );
  Some(apply_replacements(source, replacements, tracker))
}

fn lower_window_or_worker_global_scope_table(
  source: &str,
  tracker: Option<&mut RewriteTracker>,
) -> Option<String> {
  let mut replacements = Vec::new();
  let table = extract_const_object(source, "windowOrWorkerGlobalScope")?;
  let mut table_text = table.text.to_string();
  table_text = table_text.replace(
    "console: core.propNonEnumerable(\n    new console.Console((msg, level) => core.print(msg, level > 1)),\n  ),",
    "console: core.propNonEnumerable(globalScopeConsole),",
  );
  replacements.push(Replacement {
    range: table.range,
    text: format!(
      "const globalScopeConsole = new console.Console((msg, level) => core.print(msg, level > 1));\nfunction installWindowOrWorkerGlobalScope(target) {{\n  core.defineGlobalProperties(target, {table_text});\n}}"
    ),
  });

  let unstable_start = source.find(
    "const unstableForWindowOrWorkerGlobalScope = { __proto__: null };",
  )?;
  let export_start = source.find("export { unstableForWindowOrWorkerGlobalScope, windowOrWorkerGlobalScope };")?;
  let net = extract_assignment_object(
    source,
    "unstableForWindowOrWorkerGlobalScope[unstableIds.net]",
  )?;
  let node_globals = extract_assignment_object(
    source,
    "unstableForWindowOrWorkerGlobalScope[unstableIds.nodeGlobals]",
  );
  let webgpu = extract_assignment_object(
    source,
    "unstableForWindowOrWorkerGlobalScope[unstableIds.webgpu]",
  )?;
  let raw_imports = extract_assignment_object(
    source,
    "unstableForWindowOrWorkerGlobalScope[unstableIds.rawImports]",
  )?;
  let css_helpers = &source[webgpu.range.end..raw_imports.range.start];
  let mut cases = vec![format!(
    "    case unstableIds.net:\n      return {};",
    net.text
  )];
  if let Some(node_globals) = node_globals {
    cases.push(format!(
      "    case unstableIds.nodeGlobals:\n      return {};",
      node_globals.text
    ));
  }
  cases.push(format!(
    "    case unstableIds.webgpu:\n      return {};",
    webgpu.text
  ));
  cases.push(format!(
    "    case unstableIds.rawImports:\n      return {};",
    raw_imports.text
  ));
  let cases = cases.join("\n");
  replacements.push(Replacement {
    range: unstable_start..export_start,
    text: format!(
      "{css_helpers}function getUnstableWindowOrWorkerGlobalScope(id) {{\n  switch (id) {{\n{cases}\n  }}\n}}\n\n",
    ),
  });
  replacements.push(Replacement {
    range: export_start
      ..export_start
        + "export { unstableForWindowOrWorkerGlobalScope, windowOrWorkerGlobalScope };"
          .len(),
    text: "export { getUnstableWindowOrWorkerGlobalScope, globalScopeConsole, installWindowOrWorkerGlobalScope };".to_string(),
  });
  Some(apply_replacements(source, replacements, tracker))
}

fn lower_main_runtime_global_table(
  source: &str,
  tracker: Option<&mut RewriteTracker>,
) -> Option<String> {
  let table = extract_const_object(source, "mainRuntimeGlobalProperties")?;
  let mut replacements = vec![Replacement {
    range: table.range,
    text: format!(
      "function installMainRuntimeGlobalProperties(target, locationHref) {{\n  const props = {};\n  if (locationHref == null) {{\n    props.location = {{\n      writable: true,\n      configurable: true,\n    }};\n  }} else {{\n    location.setLocationHref(locationHref);\n  }}\n  core.defineGlobalProperties(target, props);\n}}",
      table.text
    ),
  }];
  let export = "export { mainRuntimeGlobalProperties, memoizeLazy };";
  if let Some(start) = source.find(export) {
    replacements.push(Replacement {
      range: start..start + export.len(),
      text: "export { installMainRuntimeGlobalProperties, memoizeLazy };"
        .to_string(),
    });
  }
  Some(apply_replacements(source, replacements, tracker))
}

fn lower_worker_runtime_global_table(
  source: &str,
  tracker: Option<&mut RewriteTracker>,
) -> Option<String> {
  let table = extract_const_object(source, "workerRuntimeGlobalProperties")?;
  let mut replacements = vec![Replacement {
    range: table.range,
    text: format!(
      "function installWorkerRuntimeGlobalProperties(target, workerType) {{\n  const props = {};\n  if (workerType === \"node\") {{\n    delete props.WorkerGlobalScope;\n    delete props.self;\n  }}\n  core.defineGlobalProperties(target, props);\n}}",
      table.text
    ),
  }];
  let export = "export { workerRuntimeGlobalProperties };";
  if let Some(start) = source.find(export) {
    replacements.push(Replacement {
      range: start..start + export.len(),
      text: "export { installWorkerRuntimeGlobalProperties };".to_string(),
    });
  }
  Some(apply_replacements(source, replacements, tracker))
}

fn lower_deno_ns_tables(
  source: &str,
  tracker: Option<&mut RewriteTracker>,
) -> Option<String> {
  let deno_start = source.find("const denoNs = {")?;
  let deno_define_start =
    source.find("\n\ncore.defineGlobalProperties(denoNs,")?;
  let deno_define_end = find_call_statement_end(source, deno_define_start + 2)?;
  let deno_block = &source[deno_start..deno_define_end];

  let unstable_start =
    source.find("const denoNsUnstableById = { __proto__: null };")?;
  let export = "export { denoNs, denoNsUnstableById, unstableIds };";
  let export_start = source.find(export)?;

  let bundle = extract_assignment_object(
    source,
    "denoNsUnstableById[unstableIds.bundle]",
  )?;
  let cron =
    extract_assignment_object(source, "denoNsUnstableById[unstableIds.cron]")?;
  let kv =
    extract_assignment_object(source, "denoNsUnstableById[unstableIds.kv]")?;
  let net =
    extract_assignment_object(source, "denoNsUnstableById[unstableIds.net]")?;
  let net_props_start = source
    .find("core.defineGlobalProperties(denoNsUnstableById[unstableIds.net],")?;
  let net_props_end = find_call_statement_end(source, net_props_start)?;
  let net_props =
    extract_first_object_argument(&source[net_props_start..net_props_end])?;
  let webgpu = extract_assignment_object(
    source,
    "denoNsUnstableById[unstableIds.webgpu]",
  )?;
  let webgpu_props_start = source.find(
    "core.defineGlobalProperties(denoNsUnstableById[unstableIds.webgpu],",
  )?;
  let webgpu_props_end = find_call_statement_end(source, webgpu_props_start)?;
  let webgpu_props = extract_first_object_argument(
    &source[webgpu_props_start..webgpu_props_end],
  )?;

  let replacements = vec![
    Replacement {
      range: deno_start..deno_define_end,
      text: format!(
        "function createDenoNs() {{\n  {deno_block}\n  return denoNs;\n}}"
      ),
    },
    Replacement {
      range: unstable_start..export_start,
      text: format!(
        "function getDenoNsUnstableById(id) {{\n  switch (id) {{\n    case unstableIds.bundle:\n      return {};\n    case unstableIds.cron:\n      return {};\n    case unstableIds.kv:\n      return {};\n    case unstableIds.net: {{\n      const unstable = {};\n      core.defineGlobalProperties(unstable, {});\n      return unstable;\n    }}\n    case unstableIds.webgpu: {{\n      const unstable = {};\n      core.defineGlobalProperties(unstable, {});\n      return unstable;\n    }}\n  }}\n}}\n\n",
        bundle.text,
        cron.text,
        kv.text,
        net.text,
        net_props,
        webgpu.text,
        webgpu_props
      ),
    },
    Replacement {
      range: export_start..export_start + export.len(),
      text: "export { createDenoNs, getDenoNsUnstableById, unstableIds };"
        .to_string(),
    },
  ];
  Some(apply_replacements(source, replacements, tracker))
}

fn lower_runtime_main_global_table_uses(
  source: &str,
  tracker: Option<&mut RewriteTracker>,
) -> Option<String> {
  let mut replacements = Vec::new();
  push_literal_replacement(
    source,
    &mut replacements,
    "import {\n  denoNs,\n  denoNsUnstableById,\n  unstableIds,\n} from \"ext:runtime/90_deno_ns.js\";",
    "import {\n  createDenoNs,\n  getDenoNsUnstableById,\n  unstableIds,\n} from \"ext:runtime/90_deno_ns.js\";",
  );
  push_literal_replacement(
    source,
    &mut replacements,
    "import {\n  unstableForWindowOrWorkerGlobalScope,\n  windowOrWorkerGlobalScope,\n} from \"ext:runtime/98_global_scope_shared.js\";",
    "import {\n  getUnstableWindowOrWorkerGlobalScope,\n  installWindowOrWorkerGlobalScope,\n} from \"ext:runtime/98_global_scope_shared.js\";",
  );
  push_literal_replacement(
    source,
    &mut replacements,
    "import {\n  mainRuntimeGlobalProperties,\n  memoizeLazy,\n} from \"ext:runtime/98_global_scope_window.js\";",
    "import {\n  installMainRuntimeGlobalProperties,\n  memoizeLazy,\n} from \"ext:runtime/98_global_scope_window.js\";",
  );
  push_literal_replacement(
    source,
    &mut replacements,
    "import {\n  workerRuntimeGlobalProperties,\n} from \"ext:runtime/98_global_scope_worker.js\";",
    "import {\n  installWorkerRuntimeGlobalProperties,\n} from \"ext:runtime/98_global_scope_worker.js\";",
  );
  push_literal_replacement(
    source,
    &mut replacements,
    "core.defineGlobalProperties(globalThis, windowOrWorkerGlobalScope);",
    "installWindowOrWorkerGlobalScope(globalThis);",
  );
  replacements.push(replace_expose_unstable_window_worker(source)?);
  replacements.push(replace_final_deno_ns(source)?);
  push_literal_replacement(
    source,
    &mut replacements,
    "denoNs.build.standalone = standalone;",
    "core.build.standalone = standalone;",
  );
  replacements.push(replace_main_runtime_global_install(source)?);
  replacements.push(replace_worker_runtime_global_install(source)?);
  push_literal_replacement(
    source,
    &mut replacements,
    "      const unstable = denoNsUnstableById[id];",
    "      const unstable = getDenoNsUnstableById(id);",
  );
  Some(apply_replacements(source, replacements, tracker))
}

fn replace_main_runtime_global_install(source: &str) -> Option<Replacement> {
  let start = source.find(
    "    if (location_ == null) {\n      mainRuntimeGlobalProperties.location = {",
  )?;
  let define =
    "    core.defineGlobalProperties(globalThis, mainRuntimeGlobalProperties);";
  let define_start = source[start..].find(define)? + start;
  Some(Replacement {
    range: start..define_start + define.len(),
    text: "    installMainRuntimeGlobalProperties(globalThis, location_);"
      .to_string(),
  })
}

fn replace_worker_runtime_global_install(source: &str) -> Option<Replacement> {
  let start = source.find(
    "    if (workerType === \"node\") {\n      delete workerRuntimeGlobalProperties[\"WorkerGlobalScope\"];",
  )?;
  let define =
    "    ObjectDefineProperties(globalThis, workerRuntimeGlobalProperties);";
  let define_start = source[start..].find(define)? + start;
  Some(Replacement {
    range: start..define_start + define.len(),
    text: "    installWorkerRuntimeGlobalProperties(globalThis, workerType);"
      .to_string(),
  })
}

fn replace_expose_unstable_window_worker(source: &str) -> Option<Replacement> {
  let start = source.find("function exposeUnstableFeaturesForWindowOrWorkerGlobalScope(unstableFeatures) {")?;
  let end = find_function_block_end(source, start)?;
  let replacement = r#"function exposeUnstableFeaturesForWindowOrWorkerGlobalScope(unstableFeatures) {
  for (let i = 0; i <= unstableFeatures.length; i++) {
    const featureId = unstableFeatures[i];
    const props = getUnstableWindowOrWorkerGlobalScope(featureId);
    if (props) {
      core.defineGlobalProperties(globalThis, { ...props });
    }
  }
}"#;
  Some(Replacement {
    range: start..end,
    text: replacement.to_string(),
  })
}

fn replace_final_deno_ns(source: &str) -> Option<Replacement> {
  let start = source.find("const finalDenoNs = ObjectDefineProperties(")?;
  let marker = "\n\nObjectDefineProperties(finalDenoNs, {";
  let end = source[start..].find(marker)? + start;
  let old = &source[start..end];
  let inner = old
    .strip_prefix("const finalDenoNs = ObjectDefineProperties(\n")?
    .strip_suffix("\n);")?;
  Some(Replacement {
    range: start..end,
    text: format!(
      "const finalDenoNs = (() => {{\n  const denoNs = createDenoNs();\n  return ObjectDefineProperties(\n{inner}\n  );\n}})();"
    ),
  })
}

struct ExtractedObject<'a> {
  range: Range<usize>,
  text: &'a str,
}

fn extract_const_object<'a>(
  source: &'a str,
  name: &str,
) -> Option<ExtractedObject<'a>> {
  let prefix = format!("const {name} = ");
  let start = source.find(&prefix)?;
  let object_start = start + prefix.len();
  let end = find_object_literal_end(source, object_start)?;
  Some(ExtractedObject {
    range: start..end,
    text: &source[object_start..end - 1],
  })
}

fn extract_assignment_object<'a>(
  source: &'a str,
  target: &str,
) -> Option<ExtractedObject<'a>> {
  let prefix = format!("{target} = ");
  let start = source.find(&prefix)?;
  let object_start = start + prefix.len();
  let end = find_object_literal_end(source, object_start)?;
  Some(ExtractedObject {
    range: start..end,
    text: &source[object_start..end - 1],
  })
}

fn extract_first_object_argument(source: &str) -> Option<&str> {
  let comma = source.find(',')?;
  let object_start = source[comma + 1..].find('{')? + comma + 1;
  let object_end = find_balanced_block_end(source, object_start)?;
  Some(&source[object_start..object_end])
}

fn find_call_statement_end(source: &str, start: usize) -> Option<usize> {
  let mut paren_depth = 0usize;
  let mut saw_open = false;
  for (offset, byte) in source[start..].bytes().enumerate() {
    match byte {
      b'(' => {
        saw_open = true;
        paren_depth += 1;
      }
      b')' if saw_open => {
        paren_depth = paren_depth.checked_sub(1)?;
        if paren_depth == 0 {
          let mut end = start + offset + 1;
          while end < source.len() && source.as_bytes()[end] != b';' {
            end += 1;
          }
          return (end < source.len()).then_some(end + 1);
        }
      }
      _ => {}
    }
  }
  None
}

fn find_function_block_end(source: &str, start: usize) -> Option<usize> {
  let brace = source[start..].find('{')? + start;
  find_balanced_block_end(source, brace)
}

fn find_object_literal_end(source: &str, start: usize) -> Option<usize> {
  if source.as_bytes().get(start) != Some(&b'{') {
    return None;
  }
  let end = find_balanced_block_end(source, start)?;
  let mut cursor = end;
  while cursor < source.len() && source.as_bytes()[cursor].is_ascii_whitespace()
  {
    cursor += 1;
  }
  if source.as_bytes().get(cursor) == Some(&b';') {
    cursor += 1;
  }
  Some(cursor)
}

fn find_balanced_block_end(source: &str, start: usize) -> Option<usize> {
  let mut depth = 0usize;
  let mut string_quote = None;
  let mut escape = false;
  let mut template_depth = 0usize;
  let mut line_comment = false;
  let mut block_comment = false;
  let bytes = source[start..].as_bytes();
  for (offset, byte) in bytes.iter().copied().enumerate() {
    if line_comment {
      if byte == b'\n' {
        line_comment = false;
      }
      continue;
    }
    if block_comment {
      if byte == b'/' && offset > 0 && bytes[offset - 1] == b'*' {
        block_comment = false;
      }
      continue;
    }
    if let Some(quote) = string_quote {
      if escape {
        escape = false;
      } else if byte == b'\\' {
        escape = true;
      } else if byte == quote {
        string_quote = None;
      } else if quote == b'`' && byte == b'{' {
        template_depth += 1;
      } else if quote == b'`' && byte == b'}' && template_depth > 0 {
        template_depth -= 1;
      }
      continue;
    }
    if byte == b'/' {
      match bytes.get(offset + 1).copied() {
        Some(b'/') => {
          line_comment = true;
          continue;
        }
        Some(b'*') => {
          block_comment = true;
          continue;
        }
        _ => {}
      }
    }
    match byte {
      b'\'' | b'"' | b'`' => string_quote = Some(byte),
      b'{' => depth += 1,
      b'}' => {
        depth = depth.checked_sub(1)?;
        if depth == 0 {
          return Some(start + offset + 1);
        }
      }
      _ => {}
    }
  }
  None
}

fn maybe_lower_lazy_descriptor_selectors(
  specifier: &str,
  source: ModuleCodeString,
  tracker: Option<&mut RewriteTracker>,
) -> ModuleCodeString {
  if !matches!(
    specifier,
    "ext:runtime/90_deno_ns.js"
      | "ext:runtime/98_global_scope_shared.js"
      | "ext:runtime/98_global_scope_window.js"
  ) {
    return source;
  }
  let source_text = source.to_string();
  if !source_text.contains("propWritableLazyLoaded")
    && !source_text.contains("propNonEnumerableLazyLoaded")
  {
    return source;
  }
  let Some(parsed) = parse_snapshot_source(specifier, &source_text) else {
    return source;
  };
  let replacements =
    collect_lazy_descriptor_selector_rewrites(&parsed, &source_text);
  if replacements.is_empty() {
    return source;
  }
  apply_replacements(&source_text, replacements, tracker).into()
}

fn collect_lazy_descriptor_selector_rewrites(
  parsed: &deno_ast::ParsedSource,
  source: &str,
) -> Vec<Replacement> {
  struct Collector<'a> {
    text_info: &'a SourceTextInfo,
    source: &'a str,
    replacements: Vec<Replacement>,
  }

  impl Visit for Collector<'_> {
    noop_visit_type!();

    fn visit_key_value_prop(&mut self, node: &KeyValueProp) {
      if prop_name(&node.key).is_none() {
        return;
      }
      let Expr::Call(call) = &*node.value else {
        return;
      };
      let Some((enumerable, load_key, load_fn_range)) =
        lazy_descriptor_call(call, self.text_info)
      else {
        return;
      };
      let load_fn = &self.source[load_fn_range];
      self.replacements.push(Replacement {
        range: byte_range(self.text_info, call.range()),
        text: format!(
          "core.propLazyLoadedByKey(\"{load_key}\", {load_fn}, {enumerable})"
        ),
      });
    }
  }

  let text_info = parsed.text_info_lazy();
  let mut collector = Collector {
    text_info,
    source,
    replacements: Vec::new(),
  };
  parsed.program_ref().visit_with(&mut collector);
  collector.replacements
}

fn lazy_descriptor_call(
  call: &CallExpr,
  text_info: &SourceTextInfo,
) -> Option<(bool, String, Range<usize>)> {
  let Callee::Expr(callee) = &call.callee else {
    return None;
  };
  let Expr::Member(member) = &**callee else {
    return None;
  };
  if !matches!(&*member.obj, Expr::Ident(ident) if ident.sym == "core") {
    return None;
  }
  let method = member_prop_name(&member.prop)?;
  let enumerable = match method.as_str() {
    "propWritableLazyLoaded" => true,
    "propNonEnumerableLazyLoaded" => false,
    _ => return None,
  };
  if call.args.len() != 2 {
    return None;
  }
  let load_key = arrow_member_key(&call.args[0].expr)?;
  let load_fn_range = byte_range(text_info, call.args[1].expr.range());
  Some((enumerable, load_key, load_fn_range))
}

fn arrow_member_key(expr: &Expr) -> Option<String> {
  let Expr::Arrow(arrow) = expr else {
    return None;
  };
  if arrow.params.len() != 1 {
    return None;
  }
  let Pat::Ident(param) = &arrow.params[0] else {
    return None;
  };
  let BlockStmtOrExpr::Expr(body) = &*arrow.body else {
    return None;
  };
  let Expr::Member(member) = &**body else {
    return None;
  };
  if !matches!(&*member.obj, Expr::Ident(ident) if ident.sym == param.id.sym) {
    return None;
  }
  member_prop_name(&member.prop)
}

fn maybe_lower_node_errors(
  specifier: &str,
  source: ModuleCodeString,
  tracker: Option<&mut RewriteTracker>,
) -> ModuleCodeString {
  if specifier == NODE_ERRORS_SPECIFIER {
    return lower_node_errors_module(specifier, source, tracker);
  }
  if !specifier.starts_with("ext:deno_node/") && !specifier.starts_with("node:")
  {
    return source;
  }
  if !source.contains("new ERR_") && !source.contains("new codes.ERR_") {
    return source;
  }
  let source_text = source.to_string();
  let names = lowered_node_error_names();
  if names.is_empty() {
    return source;
  }
  let Some(parsed) = parse_snapshot_source(specifier, &source_text) else {
    return source;
  };
  let mut replacements =
    collect_node_error_new_rewrites(&parsed, &source_text, names);
  if replacements.is_empty() {
    return source;
  }
  if !source_text.contains(NODE_ERROR_HELPER_NAME) {
    if let Some(offset) = helper_import_offset(specifier, &source_text) {
      replacements.push(Replacement {
        range: offset..offset,
        text: format!(
          "const {{ {NODE_ERROR_HELPER_NAME} }} = core.loadExtScript(\"{NODE_ERRORS_SPECIFIER}\");\n",
        ),
      });
    }
  }
  apply_replacements(&source_text, replacements, tracker).into()
}

fn lower_node_errors_module(
  specifier: &str,
  source: ModuleCodeString,
  tracker: Option<&mut RewriteTracker>,
) -> ModuleCodeString {
  let source_text = source.to_string();
  let Some(parsed) = parse_snapshot_source(specifier, &source_text) else {
    return source;
  };
  let lowered = collect_lowered_node_errors(&parsed, &source_text);
  if lowered.is_empty() {
    return source;
  }
  let names: HashSet<String> =
    lowered.iter().map(|entry| entry.name.clone()).collect();
  let mut replacements = Vec::new();
  for entry in &lowered {
    replacements.push(Replacement {
      range: entry.start..entry.end,
      text: String::new(),
    });
  }
  replacements.extend(collect_node_error_export_rewrites(&parsed, &names));
  replacements.extend(collect_node_error_code_assignment_removals(
    &parsed,
    &source_text,
    &names,
  ));
  replacements.extend(collect_node_error_code_assignment_line_removals(
    &source_text,
    &names,
  ));
  replacements.extend(collect_node_error_define_helper_removals(
    &parsed,
    &source_text,
  ));
  replacements.extend(collect_node_error_new_rewrites(
    &parsed,
    &source_text,
    &names,
  ));
  replacements.push(Replacement {
    range: lowered[0].start..lowered[0].start,
    text: format!("{}\n\n", shared_node_error_helper_text(&lowered)),
  });
  apply_replacements(&source_text, replacements, tracker).into()
}

fn parse_snapshot_source(
  specifier: &str,
  source: &str,
) -> Option<deno_ast::ParsedSource> {
  let media_type =
    if specifier.ends_with(".ts") || specifier.starts_with("node:") {
      MediaType::TypeScript
    } else {
      MediaType::JavaScript
    };
  deno_ast::parse_module(ParseParams {
    specifier: deno_core::url::Url::parse(specifier)
      .or_else(|_| deno_core::url::Url::parse(&format!("ext:///{specifier}")))
      .ok()?,
    text: source.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })
  .ok()
}

fn lowered_node_error_names() -> &'static HashSet<String> {
  static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
  NAMES.get_or_init(|| {
    let source = include_str!("../ext/node/polyfills/internal/errors.ts");
    let Some(parsed) = parse_snapshot_source(NODE_ERRORS_SPECIFIER, source)
    else {
      return HashSet::new();
    };
    collect_lowered_node_errors(&parsed, source)
      .into_iter()
      .map(|entry| entry.name)
      .collect()
  })
}

fn collect_lowered_node_errors(
  parsed: &deno_ast::ParsedSource,
  source: &str,
) -> Vec<LoweredNodeError> {
  struct Collector<'a> {
    text_info: &'a SourceTextInfo,
    source: &'a str,
    lowered: Vec<LoweredNodeError>,
  }

  impl Visit for Collector<'_> {
    noop_visit_type!();

    fn visit_stmt(&mut self, node: &Stmt) {
      if let Some(entry) = collect_lowerable_node_error_definition(
        node,
        self.text_info,
        self.source,
      ) {
        self.lowered.push(entry);
        return;
      }
      if let Some(entry) =
        collect_lowerable_node_error_class(node, self.text_info, self.source)
      {
        self.lowered.push(entry);
        return;
      }
      node.visit_children_with(self);
    }
  }

  let text_info = parsed.text_info_lazy();
  let mut collector = Collector {
    text_info,
    source,
    lowered: Vec::new(),
  };
  parsed.program_ref().visit_with(&mut collector);
  collector.lowered
}

fn collect_lowerable_node_error_definition(
  node: &Stmt,
  text_info: &SourceTextInfo,
  source: &str,
) -> Option<LoweredNodeError> {
  let Stmt::Decl(Decl::Var(var_decl)) = node else {
    return None;
  };
  if var_decl.decls.len() != 1 {
    return None;
  }
  let declarator = &var_decl.decls[0];
  let Pat::Ident(ident) = &declarator.name else {
    return None;
  };
  let name = ident.id.sym.to_string();
  if !name.starts_with("ERR_") {
    return None;
  }
  let Expr::Call(call) = &**declarator.init.as_ref()? else {
    return None;
  };
  let Callee::Expr(callee) = &call.callee else {
    return None;
  };
  if !matches!(&**callee, Expr::Ident(callee) if callee.sym == NODE_ERROR_DEFINE_NAME)
  {
    return None;
  }
  if call.args.len() != 3 {
    return None;
  }
  let Expr::Lit(Lit::Str(code)) = &*call.args[0].expr else {
    return None;
  };
  if code.value.to_string_lossy() != name {
    return None;
  }
  let Expr::Ident(base_ident) = &*call.args[1].expr else {
    return None;
  };
  let base = base_ident.sym.to_string();
  if !is_lowerable_node_error_base(&base) {
    return None;
  }
  let Expr::Arrow(formatter) = &*call.args[2].expr else {
    return None;
  };
  let BlockStmtOrExpr::Expr(message) = &*formatter.body else {
    return None;
  };
  let mut params = Vec::with_capacity(formatter.params.len());
  for param in &formatter.params {
    params.push(pat_binding(param, text_info, source)?);
  }
  let message_range = byte_range(text_info, message.range());
  let declaration_range =
    expand_to_whole_line(source, byte_range(text_info, node.range()));
  Some(LoweredNodeError {
    name,
    base,
    start: declaration_range.range.start,
    end: declaration_range.range.end,
    params,
    message_prelude: String::new(),
    message_expression: source[message_range].to_string(),
    message_postlude: String::new(),
  })
}

fn collect_lowerable_node_error_class(
  node: &Stmt,
  text_info: &SourceTextInfo,
  source: &str,
) -> Option<LoweredNodeError> {
  let Stmt::Decl(Decl::Class(class_decl)) = node else {
    return None;
  };
  let name = class_decl.ident.sym.to_string();
  if !name.starts_with("ERR_") {
    return None;
  }
  if !is_lowerable_node_error_class_name(&name) {
    return None;
  }
  let Expr::Ident(base_ident) = &**class_decl.class.super_class.as_ref()?
  else {
    return None;
  };
  let base = base_ident.sym.to_string();
  if !is_lowerable_node_error_base(&base) {
    return None;
  }

  let mut constructor = None;
  for member in &class_decl.class.body {
    match member {
      ClassMember::Constructor(node) => {
        if constructor.replace(node).is_some() {
          return None;
        }
      }
      ClassMember::ClassProp(prop)
        if is_hide_stack_frames_static_alias(prop) => {}
      ClassMember::ClassProp(prop)
        if prop.value.is_none() && !prop.is_static => {}
      ClassMember::Empty(_) => {}
      _ => return None,
    }
  }
  let constructor = constructor?;
  let body = constructor.body.as_ref()?;
  let (super_index, call) = find_node_error_super_call(&body.stmts)?;
  let Expr::Lit(Lit::Str(code)) = &*call.args[0].expr else {
    return None;
  };
  if code.value.to_string_lossy() != name {
    return None;
  }

  let mut params = Vec::with_capacity(constructor.params.len());
  for param in &constructor.params {
    let ParamOrTsParamProp::Param(param) = param else {
      return None;
    };
    params.push(pat_binding(&param.pat, text_info, source)?);
  }

  let message_range = byte_range(text_info, call.args[1].expr.range());
  if !body.stmts[super_index + 1..]
    .iter()
    .all(is_lowerable_node_error_this_assignment)
  {
    return None;
  }

  let message_prelude = if super_index == 0 {
    String::new()
  } else {
    let prelude_start = byte_range(text_info, body.stmts[0].range()).start;
    let prelude_end =
      byte_range(text_info, body.stmts[super_index - 1].range()).end;
    source[prelude_start..prelude_end].to_string()
  };
  let message_postlude = if super_index + 1 >= body.stmts.len() {
    String::new()
  } else {
    let postlude_start =
      byte_range(text_info, body.stmts[super_index + 1].range()).start;
    let postlude_end =
      byte_range(text_info, body.stmts[body.stmts.len() - 1].range()).end;
    source[postlude_start..postlude_end].replace("this.", "error.")
  };
  let declaration_range =
    expand_to_whole_line(source, byte_range(text_info, node.range()));
  Some(LoweredNodeError {
    name,
    base,
    start: declaration_range.range.start,
    end: declaration_range.range.end,
    params,
    message_prelude,
    message_expression: source[message_range].to_string(),
    message_postlude,
  })
}

fn find_node_error_super_call(stmts: &[Stmt]) -> Option<(usize, &CallExpr)> {
  let mut found = None;
  for (index, stmt) in stmts.iter().enumerate() {
    let Stmt::Expr(expr_stmt) = stmt else {
      continue;
    };
    let Expr::Call(call) = &*expr_stmt.expr else {
      continue;
    };
    if !matches!(call.callee, Callee::Super(_)) || call.args.len() != 2 {
      continue;
    }
    if found.replace((index, call)).is_some() {
      return None;
    }
  }
  found
}

fn is_lowerable_node_error_this_assignment(stmt: &Stmt) -> bool {
  let Stmt::Expr(expr_stmt) = stmt else {
    return false;
  };
  let Expr::Assign(assign) = &*expr_stmt.expr else {
    return false;
  };
  let AssignTarget::Simple(SimpleAssignTarget::Member(member)) = &assign.left
  else {
    return false;
  };
  matches!(&*member.obj, Expr::This(_))
}

fn is_hide_stack_frames_static_alias(prop: &ClassProp) -> bool {
  prop.is_static
    && prop_name(&prop.key).as_deref() == Some("HideStackFramesError")
    && matches!(prop.value.as_deref(), Some(Expr::This(_)))
}

fn is_lowerable_node_error_base(base: &str) -> bool {
  matches!(
    base,
    "NodeError"
      | "NodeRangeError"
      | "NodeSyntaxError"
      | "NodeTypeError"
      | "NodeURIError"
  )
}

fn is_lowerable_node_error_class_name(name: &str) -> bool {
  matches!(
    name,
    "ERR_INVALID_PACKAGE_CONFIG"
      | "ERR_INVALID_PACKAGE_TARGET"
      | "ERR_INVALID_ADDRESS_FAMILY"
      | "ERR_INVALID_FILE_URL_PATH"
      | "ERR_INVALID_MIME_SYNTAX"
      | "ERR_INVALID_URL"
      | "ERR_INVALID_URL_SCHEME"
      | "ERR_PACKAGE_IMPORT_NOT_DEFINED"
      | "ERR_PACKAGE_PATH_NOT_EXPORTED"
      | "ERR_FALSY_VALUE_REJECTION"
      | "ERR_HTTP2_INVALID_HEADER_VALUE"
      | "ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED"
      | "ERR_PARSE_ARGS_UNKNOWN_OPTION"
      | "ERR_INVALID_HTTP_TOKEN"
      | "ERR_REQUIRE_ASYNC_MODULE"
      | "ERR_REQUIRE_CYCLE_MODULE"
      | "ERR_TLS_CERT_ALTNAME_INVALID"
      | "ERR_WORKER_PATH"
  )
}

fn pat_binding(
  pat: &Pat,
  text_info: &SourceTextInfo,
  source: &str,
) -> Option<String> {
  match pat {
    Pat::Ident(ident) => Some(ident.id.sym.to_string()),
    Pat::Assign(assign) => {
      let Pat::Ident(left) = &*assign.left else {
        return None;
      };
      let right_range = byte_range(text_info, assign.right.range());
      Some(format!("{} = {}", left.id.sym, &source[right_range],))
    }
    _ => None,
  }
}

fn collect_node_error_define_helper_removals(
  parsed: &deno_ast::ParsedSource,
  source: &str,
) -> Vec<Replacement> {
  struct Collector<'a> {
    text_info: &'a SourceTextInfo,
    source: &'a str,
    replacements: Vec<Replacement>,
  }

  impl Visit for Collector<'_> {
    noop_visit_type!();

    fn visit_fn_decl(&mut self, node: &FnDecl) {
      if node.ident.sym != NODE_ERROR_DEFINE_NAME {
        return;
      }
      let range = byte_range(self.text_info, node.range());
      self
        .replacements
        .push(expand_to_whole_line(self.source, range));
    }
  }

  let text_info = parsed.text_info_lazy();
  let mut collector = Collector {
    text_info,
    source,
    replacements: Vec::new(),
  };
  parsed.program_ref().visit_with(&mut collector);
  collector.replacements
}

fn collect_node_error_export_rewrites(
  parsed: &deno_ast::ParsedSource,
  lowered_names: &HashSet<String>,
) -> Vec<Replacement> {
  struct Collector<'a> {
    text_info: &'a SourceTextInfo,
    lowered_names: &'a HashSet<String>,
    replacements: Vec<Replacement>,
  }

  impl Visit for Collector<'_> {
    noop_visit_type!();

    fn visit_return_stmt(&mut self, node: &ReturnStmt) {
      let Some(argument) = &node.arg else {
        return;
      };
      let Expr::Object(object) = &**argument else {
        return;
      };
      if !object.props.iter().any(|prop| match prop {
        PropOrSpread::Prop(prop) => match &**prop {
          Prop::Shorthand(ident) => ident.sym == "AbortError",
          _ => false,
        },
        PropOrSpread::Spread(_) => false,
      }) {
        return;
      }
      let return_range = byte_range(self.text_info, node.range());
      self.replacements.push(Replacement {
        range: return_range.start..return_range.start + "return".len(),
        text: "const errors =".to_string(),
      });
      self.replacements.push(Replacement {
        range: return_range.end..return_range.end,
        text: "\nerrors.default = errors;\nreturn errors;".to_string(),
      });
      for prop in &object.props {
        let PropOrSpread::Prop(prop) = prop else {
          continue;
        };
        match &**prop {
          Prop::Shorthand(ident)
            if self.lowered_names.contains(&ident.sym.to_string()) =>
          {
            let range = byte_range(self.text_info, ident.range());
            self
              .replacements
              .push(expand_to_whole_line(self.text_info.text_str(), range));
          }
          Prop::KeyValue(key_value)
            if prop_name(&key_value.key).as_deref() == Some("default") =>
          {
            let range = byte_range(self.text_info, key_value.range());
            self
              .replacements
              .push(expand_to_whole_line(self.text_info.text_str(), range));
          }
          _ => {}
        }
      }
      for prop in &object.props {
        let PropOrSpread::Prop(prop) = prop else {
          continue;
        };
        let Prop::Shorthand(ident) = &**prop else {
          continue;
        };
        if ident.sym != "AbortError" {
          continue;
        }
        let range = byte_range(self.text_info, ident.range());
        let indent = indentation_at(self.text_info.text_str(), range.start);
        self.replacements.push(Replacement {
          range: range.start..range.start,
          text: format!("{indent}{NODE_ERROR_HELPER_NAME},\n"),
        });
        break;
      }
    }
  }

  let text_info = parsed.text_info_lazy();
  let mut collector = Collector {
    text_info,
    lowered_names,
    replacements: Vec::new(),
  };
  parsed.program_ref().visit_with(&mut collector);
  collector.replacements
}

fn collect_node_error_code_assignment_removals(
  parsed: &deno_ast::ParsedSource,
  source: &str,
  lowered_names: &HashSet<String>,
) -> Vec<Replacement> {
  struct Collector<'a> {
    text_info: &'a SourceTextInfo,
    source: &'a str,
    lowered_names: &'a HashSet<String>,
    replacements: Vec<Replacement>,
  }

  impl Visit for Collector<'_> {
    noop_visit_type!();

    fn visit_stmt(&mut self, node: &Stmt) {
      let Stmt::Expr(expr_stmt) = node else {
        return;
      };
      let Expr::Assign(assign) = &*expr_stmt.expr else {
        return;
      };
      let AssignTarget::Simple(SimpleAssignTarget::Member(member)) =
        &assign.left
      else {
        return;
      };
      if !matches!(&*member.obj, Expr::Ident(ident) if ident.sym == "codes") {
        return;
      }
      let Some(name) = member_prop_name(&member.prop) else {
        return;
      };
      if self.lowered_names.contains(&name) {
        let range = byte_range(self.text_info, node.range());
        self
          .replacements
          .push(expand_to_whole_line(self.source, range));
      }
    }
  }

  let text_info = parsed.text_info_lazy();
  let mut collector = Collector {
    text_info,
    source,
    lowered_names,
    replacements: Vec::new(),
  };
  parsed.program_ref().visit_with(&mut collector);
  collector.replacements
}

fn collect_node_error_code_assignment_line_removals(
  source: &str,
  lowered_names: &HashSet<String>,
) -> Vec<Replacement> {
  let mut replacements = Vec::new();
  let lines = source.split_inclusive('\n').collect::<Vec<_>>();
  let mut offsets = Vec::with_capacity(lines.len());
  let mut cursor = 0;
  for line in &lines {
    offsets.push(cursor);
    cursor += line.len();
  }
  let mut index = 0;
  while index < lines.len() {
    let line = lines[index];
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("codes.") {
      if let Some((name, rhs)) = rest.split_once(" =") {
        if lowered_names.contains(name) {
          let start = offsets[index];
          let mut end = offsets[index] + line.len();
          let mut rhs_text = rhs;
          while !rhs_text.contains(';') && index + 1 < lines.len() {
            index += 1;
            rhs_text = lines[index];
            end = offsets[index] + rhs_text.len();
          }
          replacements.push(Replacement {
            range: start..end,
            text: String::new(),
          });
        }
      }
    }
    index += 1;
  }
  replacements
}

fn collect_node_error_new_rewrites(
  parsed: &deno_ast::ParsedSource,
  source: &str,
  lowered_names: &HashSet<String>,
) -> Vec<Replacement> {
  struct Collector<'a> {
    text_info: &'a SourceTextInfo,
    source: &'a str,
    lowered_names: &'a HashSet<String>,
    replacements: Vec<Replacement>,
  }

  impl Visit for Collector<'_> {
    noop_visit_type!();

    fn visit_new_expr(&mut self, node: &NewExpr) {
      let Some(code) = new_expr_code(&node.callee, self.lowered_names) else {
        return;
      };
      let mut text = format!("{NODE_ERROR_HELPER_NAME}(\"{code}\"");
      if let Some(args) = &node.args {
        for arg in args {
          let range = byte_range(self.text_info, arg.expr.range());
          text.push_str(", ");
          text.push_str(&self.source[range]);
        }
      }
      text.push(')');
      let range = byte_range(self.text_info, node.range());
      self.replacements.push(Replacement { range, text });
    }
  }

  let text_info = parsed.text_info_lazy();
  let mut collector = Collector {
    text_info,
    source,
    lowered_names,
    replacements: Vec::new(),
  };
  parsed.program_ref().visit_with(&mut collector);
  collector.replacements
}

fn new_expr_code(
  callee: &Expr,
  lowered_names: &HashSet<String>,
) -> Option<String> {
  if let Expr::Ident(ident) = callee {
    let name = ident.sym.to_string();
    return lowered_names.contains(&name).then_some(name);
  }
  let Expr::Member(member) = callee else {
    return None;
  };
  if member_prop_name(&member.prop).as_deref() == Some("HideStackFramesError") {
    if let Expr::Ident(ident) = &*member.obj {
      let name = ident.sym.to_string();
      return lowered_names.contains(&name).then_some(name);
    }
    if let Expr::Member(inner) = &*member.obj
      && matches!(&*inner.obj, Expr::Ident(ident) if ident.sym == "codes")
    {
      let name = member_prop_name(&inner.prop)?;
      return lowered_names.contains(&name).then_some(name);
    }
  }
  if !matches!(&*member.obj, Expr::Ident(ident) if ident.sym == "codes") {
    return None;
  }
  let name = member_prop_name(&member.prop)?;
  lowered_names.contains(&name).then_some(name)
}

fn helper_import_offset(specifier: &str, source: &str) -> Option<usize> {
  if let Some(index) = source.find("__bootstrap") {
    return statement_end(specifier, source, index);
  }
  if let Some(index) = source.find("\"ext:core/mod.js\"") {
    return statement_end(specifier, source, index);
  }
  None
}

fn statement_end(
  _specifier: &str,
  source: &str,
  start: usize,
) -> Option<usize> {
  let mut cursor = start;
  while cursor < source.len() && source.as_bytes()[cursor] != b';' {
    cursor += 1;
  }
  (cursor < source.len()).then_some((cursor + 2).min(source.len()))
}

fn shared_node_error_helper_text(lowered: &[LoweredNodeError]) -> String {
  let cases = lowered
    .iter()
    .map(|entry| {
      let bindings = if entry.params.is_empty() {
        String::new()
      } else {
        format!("\n      const [{}] = args;", entry.params.join(", "))
      };
      let prelude = node_error_message_prelude(&entry.message_prelude);
      let postlude = node_error_message_postlude(&entry.message_postlude);
      if postlude.is_empty() {
        format!(
          "    case \"{}\": {{{}{}\n      return new {}(\"{}\", {});\n    }}",
          entry.name,
          bindings,
          prelude,
          entry.base,
          entry.name,
          entry.message_expression
        )
      } else {
        format!(
          "    case \"{}\": {{{}{}\n      const error = new {}(\"{}\", {});{}\n      return error;\n    }}",
          entry.name,
          bindings,
          prelude,
          entry.base,
          entry.name,
          entry.message_expression,
          postlude
        )
      }
    })
    .collect::<Vec<_>>()
    .join("\n");
  format!(
    "// deno-lint-ignore no-explicit-any\nfunction {NODE_ERROR_HELPER_NAME}(code: string, ...args: any[]) {{\n  switch (code) {{\n{cases}\n  }}\n  throw new Error(`Missing formatter for ${{code}}`);\n}}"
  )
}

fn node_error_message_prelude(prelude: &str) -> String {
  let prelude = prelude.trim();
  if prelude.is_empty() {
    return String::new();
  }
  let lines = prelude
    .lines()
    .map(|line| format!("      {}", line.trim_start()))
    .collect::<Vec<_>>()
    .join("\n");
  format!("\n{lines}")
}

fn node_error_message_postlude(postlude: &str) -> String {
  let postlude = postlude.trim();
  if postlude.is_empty() {
    return String::new();
  }
  let lines = postlude
    .lines()
    .map(|line| format!("      {}", line.trim_start()))
    .collect::<Vec<_>>()
    .join("\n");
  format!("\n{lines}")
}

fn prop_name(prop_name: &PropName) -> Option<String> {
  match prop_name {
    PropName::Ident(ident) => Some(ident.sym.to_string()),
    PropName::Str(str_) => Some(str_.value.to_string_lossy().into_owned()),
    PropName::Num(num) => Some(num.value.to_string()),
    _ => None,
  }
}

fn member_prop_name(prop: &MemberProp) -> Option<String> {
  match prop {
    MemberProp::Ident(ident) => Some(ident.sym.to_string()),
    MemberProp::Computed(computed) => match &*computed.expr {
      Expr::Lit(Lit::Str(str_)) => {
        Some(str_.value.to_string_lossy().into_owned())
      }
      _ => None,
    },
    MemberProp::PrivateName(_) => None,
  }
}

fn byte_range(
  text_info: &SourceTextInfo,
  range: deno_ast::SourceRange<deno_ast::SourcePos>,
) -> Range<usize> {
  range.as_byte_range(text_info.range().start)
}

fn expand_to_whole_line(source: &str, range: Range<usize>) -> Replacement {
  let mut start = range.start;
  while start > 0 && source.as_bytes()[start - 1] != b'\n' {
    start -= 1;
  }
  let mut end = range.end;
  while end < source.len() && source.as_bytes()[end] != b'\n' {
    end += 1;
  }
  if end < source.len() {
    end += 1;
  }
  Replacement {
    range: start..end,
    text: String::new(),
  }
}

fn apply_replacements(
  source: &str,
  mut replacements: Vec<Replacement>,
  tracker: Option<&mut RewriteTracker>,
) -> String {
  replacements.retain(|replacement| {
    !replacement.text.is_empty()
      || replacement.range.start != replacement.range.end
  });
  replacements.sort_by(|a, b| {
    a.range.start.cmp(&b.range.start).then_with(|| {
      (a.range.end - a.range.start).cmp(&(b.range.end - b.range.start))
    })
  });
  if let Some(tracker) = tracker {
    tracker.apply_replacements(source.len(), &replacements);
  }
  replacements.sort_by(|a, b| {
    b.range.start.cmp(&a.range.start).then_with(|| {
      (b.range.end - b.range.start).cmp(&(a.range.end - a.range.start))
    })
  });
  let mut updated = source.to_string();
  for replacement in replacements {
    updated.replace_range(replacement.range, &replacement.text);
  }
  updated
}

pub fn source_map_output_dir() -> Option<&'static PathBuf> {
  static OUTPUT_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
  OUTPUT_DIR
    .get_or_init(|| std::env::var_os(SOURCE_MAP_DIR_ENV).map(PathBuf::from))
    .as_ref()
}

pub fn write_build_source_map(kind: &str, specifier: &str, source_map: &[u8]) {
  let Some(output_dir) = source_map_output_dir() else {
    return;
  };
  let output_dir = output_dir.join(kind);
  std::fs::create_dir_all(&output_dir).unwrap_or_else(|err| {
    panic!(
      "failed to create snapshot optimizer source map directory {}: {err}",
      output_dir.display()
    )
  });
  let output_path =
    output_dir.join(format!("{}.map", sanitize_specifier(specifier)));
  std::fs::write(&output_path, source_map).unwrap_or_else(|err| {
    panic!(
      "failed to write snapshot optimizer source map {}: {err}",
      output_path.display()
    )
  });
}

fn sanitize_specifier(specifier: &str) -> String {
  specifier
    .chars()
    .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn lowers_node_errors_internal_module() {
    let source = include_str!("../ext/node/polyfills/internal/errors.ts");
    let lowered = lower_node_errors_module(
      NODE_ERRORS_SPECIFIER,
      source.to_string().into(),
      None,
    )
    .to_string();
    assert!(lowered.contains(NODE_ERROR_HELPER_NAME));
    assert!(lowered.contains("makeNodeErrorByCode,"));
    assert!(!lowered.contains("defineNodeError("));
    assert!(!lowered.contains("class ERR_BUFFER_TOO_LARGE"));
    assert!(!lowered.contains("class ERR_INVALID_PACKAGE_CONFIG"));
    assert!(lowered.contains("case \"ERR_INVALID_PACKAGE_CONFIG\""));
    assert!(!lowered.contains("class ERR_REQUIRE_CYCLE_MODULE"));
    assert!(
      lowered.contains("error.toString = nodeErrorToStringWithEmbeddedCode;")
    );
    assert!(!lowered.contains("class ERR_PARSE_ARGS_UNKNOWN_OPTION"));
    assert!(lowered.contains("case \"ERR_PARSE_ARGS_UNKNOWN_OPTION\""));
    assert!(!lowered.contains("class ERR_INVALID_HTTP_TOKEN"));
    assert!(lowered.contains("case \"ERR_INVALID_HTTP_TOKEN\""));
    assert!(!lowered.contains("codes.ERR_BUFFER_TOO_LARGE"));
    assert!(!lowered.contains("  ERR_BUFFER_TOO_LARGE,"));
    if let Some(index) =
      lowered.find("codes.ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL")
    {
      let start = index.saturating_sub(120);
      let end = (index + 160).min(lowered.len());
      panic!("{}", &lowered[start..end]);
    }
    assert!(!lowered.contains("  ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL,"));
  }

  #[test]
  fn lowers_node_console_esm() {
    let source = include_str!("../ext/node/polyfills/console_esm.ts");
    let lowered = lower_node_console_esm(source, None).unwrap();
    assert!(lowered.contains("globalScopeConsole"));
    assert!(lowered.contains("const console = globalScopeConsole;"));
    assert!(!lowered.contains("const _consoleDesc ="));
    assert!(!lowered.contains("import { windowOrWorkerGlobalScope }"));
    if parse_snapshot_source("node:console", &lowered).is_none() {
      panic!("{lowered}");
    }
  }

  #[test]
  fn lowers_deno_ns_tables_to_valid_source() {
    let source = include_str!("js/90_deno_ns.js");
    let lowered = lower_deno_ns_tables(source, None).unwrap();
    let lowered = maybe_lower_lazy_descriptor_selectors(
      "ext:runtime/90_deno_ns.js",
      lowered.into(),
      None,
    )
    .to_string();
    assert!(lowered.contains("core.propLazyLoadedByKey("));
    assert!(!lowered.contains("function propLazyLoadedByKey("));
    if parse_snapshot_source("ext:runtime/90_deno_ns.js", &lowered).is_none() {
      panic!("{lowered}");
    }
  }

  #[test]
  fn lowers_window_or_worker_global_scope_to_valid_source() {
    let source = include_str!("js/98_global_scope_shared.js");
    extract_const_object(source, "windowOrWorkerGlobalScope")
      .expect("windowOrWorkerGlobalScope table");
    source
      .find("const unstableForWindowOrWorkerGlobalScope = { __proto__: null };")
      .expect("unstable table declaration");
    source
      .find("export { unstableForWindowOrWorkerGlobalScope, windowOrWorkerGlobalScope };")
      .expect("shared export");
    extract_assignment_object(
      source,
      "unstableForWindowOrWorkerGlobalScope[unstableIds.net]",
    )
    .expect("net unstable table");
    assert!(
      extract_assignment_object(
        source,
        "unstableForWindowOrWorkerGlobalScope[unstableIds.nodeGlobals]",
      )
      .is_none(),
      "nodeGlobals unstable table should stay absent in current source shape",
    );
    extract_assignment_object(
      source,
      "unstableForWindowOrWorkerGlobalScope[unstableIds.webgpu]",
    )
    .expect("webgpu unstable table");
    extract_assignment_object(
      source,
      "unstableForWindowOrWorkerGlobalScope[unstableIds.rawImports]",
    )
    .expect("rawImports unstable table");
    let lowered =
      lower_window_or_worker_global_scope_table(source, None).unwrap();
    let lowered = maybe_lower_lazy_descriptor_selectors(
      "ext:runtime/98_global_scope_shared.js",
      lowered.into(),
      None,
    )
    .to_string();
    assert!(lowered.contains("globalScopeConsole"));
    assert!(lowered.contains("core.propLazyLoadedByKey("));
    assert!(!lowered.contains("function propLazyLoadedByKey("));
    assert!(
      lowered.contains("export { getUnstableWindowOrWorkerGlobalScope, globalScopeConsole, installWindowOrWorkerGlobalScope };")
    );
    if parse_snapshot_source("ext:runtime/98_global_scope_shared.js", &lowered)
      .is_none()
    {
      panic!("{lowered}");
    }
  }

  #[test]
  fn lowers_runtime_global_install_tables_to_valid_source() {
    let main_window_source = include_str!("js/98_global_scope_window.js");
    let main_window_selectors = maybe_lower_lazy_descriptor_selectors(
      "ext:runtime/98_global_scope_window.js",
      main_window_source.to_string().into(),
      None,
    )
    .to_string();
    assert!(main_window_selectors.contains("core.propLazyLoadedByKey("));
    assert!(!main_window_selectors.contains("core.propWritableLazyLoaded("));
    assert!(
      !main_window_selectors.contains("core.propNonEnumerableLazyLoaded(")
    );
    if parse_snapshot_source(
      "ext:runtime/98_global_scope_window.js",
      &main_window_selectors,
    )
    .is_none()
    {
      panic!("{main_window_selectors}");
    }

    let main_window = lower_main_runtime_global_table(main_window_source, None)
      .expect("main runtime globals");
    assert!(
      main_window.contains("function installMainRuntimeGlobalProperties(")
    );
    assert!(!main_window.contains("const mainRuntimeGlobalProperties ="));
    assert!(!main_window.contains("export { mainRuntimeGlobalProperties"));
    if parse_snapshot_source(
      "ext:runtime/98_global_scope_window.js",
      &main_window,
    )
    .is_none()
    {
      panic!("{main_window}");
    }

    let worker_source = include_str!("js/98_global_scope_worker.js");
    let worker = lower_worker_runtime_global_table(worker_source, None)
      .expect("worker runtime globals");
    assert!(worker.contains("function installWorkerRuntimeGlobalProperties("));
    assert!(!worker.contains("const workerRuntimeGlobalProperties ="));
    assert!(!worker.contains("export { workerRuntimeGlobalProperties"));
    if parse_snapshot_source("ext:runtime/98_global_scope_worker.js", &worker)
      .is_none()
    {
      panic!("{worker}");
    }
  }

  #[test]
  fn lowers_runtime_main_global_table_uses() {
    let source = include_str!("js/99_main.js");
    let lowered = lower_runtime_main_global_table_uses(source, None)
      .expect("runtime main bridge");
    for old in [
      "denoNs,\n  denoNsUnstableById,",
      "unstableForWindowOrWorkerGlobalScope",
      "windowOrWorkerGlobalScope",
      "core.defineGlobalProperties(globalThis, windowOrWorkerGlobalScope);",
      "const unstable = denoNsUnstableById[id];",
    ] {
      assert!(!lowered.contains(old), "{old}");
    }
    for new in [
      "createDenoNs",
      "getDenoNsUnstableById",
      "installWindowOrWorkerGlobalScope(globalThis);",
      "const unstable = getDenoNsUnstableById(id);",
    ] {
      assert!(lowered.contains(new), "{new}");
    }
    if parse_snapshot_source("ext:runtime_main/js/99_main.js", &lowered)
      .is_none()
    {
      panic!("{lowered}");
    }
  }

  #[test]
  fn replacement_tracker_maps_copied_and_generated_spans() {
    let source = "aaa\nbbb\nccc\n";
    let mut tracker = RewriteTracker::new(source);
    let updated = apply_replacements(
      source,
      vec![Replacement {
        range: 4..8,
        text: "XXX\nYYY\n".to_string(),
      }],
      Some(&mut tracker),
    );
    assert_eq!(updated, "aaa\nXXX\nYYY\nccc\n");
    let source_map = tracker
      .to_source_map("ext:test/source.js", &updated)
      .expect("source map");
    let source_map = sourcemap::SourceMap::from_slice(&source_map).unwrap();

    let generated = source_map.lookup_token(1, 0).unwrap();
    assert!(generated.get_source().is_none());

    let copied = source_map.lookup_token(3, 0).unwrap();
    assert_eq!(copied.get_source(), Some("ext:test/source.js"));
    assert_eq!(copied.get_src_line(), 2);
    assert_eq!(copied.get_src_col(), 0);
  }
}
