// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::analysis::ResolvedImport;
use super::state::ServerStateSnapshot;
use super::text;

use crate::media_type::MediaType;
use crate::tsc::ResolveArgs;
use crate::tsc_config::TsConfig;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::json_op_sync;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpFn;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use regex::Captures;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;

fn display_parts_to_string(
  maybe_parts: Option<Vec<SymbolDisplayPart>>,
) -> Option<String> {
  if let Some(parts) = maybe_parts {
    Some(
      parts
        .into_iter()
        .map(|p| p.text)
        .collect::<Vec<String>>()
        .join(""),
    )
  } else {
    None
  }
}

fn get_tag_body_text(tag: &JSDocTagInfo) -> Option<String> {
  if let Some(text) = &tag.text {
    match tag.name.as_str() {
      "example" => {
        let caption_regex =
          Regex::new(r"<caption>(.*?)</caption>\s*\r?\n((?:\s|\S)*)").unwrap();
        if caption_regex.is_match(text) {
          Some(
            caption_regex
              .replace(text, |c: &Captures| {
                format!("{}\n\n{}", &c[1], make_codeblock(&c[2]))
              })
              .to_string(),
          )
        } else {
          Some(make_codeblock(text))
        }
      }
      "author" => {
        let email_match_regex =
          Regex::new(r"(.+)\s<([-.\w]+@[-.\w]+)>").unwrap();
        Some(
          email_match_regex
            .replace(text, |c: &Captures| format!("{} {}", &c[1], &c[2]))
            .to_string(),
        )
      }
      "default" => Some(make_codeblock(text)),
      _ => Some(replace_links(text)),
    }
  } else {
    None
  }
}

fn get_tag_documentation(tag: &JSDocTagInfo) -> String {
  match tag.name.as_str() {
    "augments" | "extends" | "param" | "template" => {
      if let Some(text) = &tag.text {
        let part_regex = Regex::new(r"^(\S+)\s*-?\s*").unwrap();
        let body: Vec<&str> = part_regex.split(&text).collect();
        if body.len() == 3 {
          let param = body[1];
          let doc = body[2];
          let label = format!("*@{}* `{}`", tag.name, param);
          if doc.is_empty() {
            return label;
          }
          if doc.contains('\n') {
            return format!("{}  \n{}", label, replace_links(doc));
          } else {
            return format!("{} - {}", label, replace_links(doc));
          }
        }
      }
    }
    _ => (),
  }
  let label = format!("*@{}*", tag.name);
  let maybe_text = get_tag_body_text(tag);
  if let Some(text) = maybe_text {
    if text.contains('\n') {
      format!("{}  \n{}", label, text)
    } else {
      format!("{} - {}", label, text)
    }
  } else {
    label
  }
}

fn make_codeblock(text: &str) -> String {
  let codeblock_regex = Regex::new(r"^\s*[~`]{3}").unwrap();
  if codeblock_regex.is_match(text) {
    text.to_string()
  } else {
    format!("```\n{}\n```", text)
  }
}

/// Replace JSDoc like links (`{@link http://example.com}`) with markdown links
fn replace_links(text: &str) -> String {
  let jsdoc_links_regex = Regex::new(r"(?i)\{@(link|linkplain|linkcode) (https?://[^ |}]+?)(?:[| ]([^{}\n]+?))?\}").unwrap();
  jsdoc_links_regex
    .replace_all(text, |c: &Captures| match &c[1] {
      "linkcode" => format!(
        "[`{}`]({})",
        if c.get(3).is_none() {
          &c[2]
        } else {
          c[3].trim()
        },
        &c[2]
      ),
      _ => format!(
        "[{}]({})",
        if c.get(3).is_none() {
          &c[2]
        } else {
          c[3].trim()
        },
        &c[2]
      ),
    })
    .to_string()
}

#[derive(Debug, Deserialize)]
pub enum ScriptElementKind {
  #[serde(rename = "")]
  Unknown,
  #[serde(rename = "warning")]
  Warning,
  #[serde(rename = "keyword")]
  Keyword,
  #[serde(rename = "script")]
  ScriptElement,
  #[serde(rename = "module")]
  ModuleElement,
  #[serde(rename = "class")]
  ClassElement,
  #[serde(rename = "local class")]
  LocalClassElement,
  #[serde(rename = "interface")]
  InterfaceElement,
  #[serde(rename = "type")]
  TypeElement,
  #[serde(rename = "enum")]
  EnumElement,
  #[serde(rename = "enum member")]
  EnumMemberElement,
  #[serde(rename = "var")]
  VariableElement,
  #[serde(rename = "local var")]
  LocalVariableElement,
  #[serde(rename = "function")]
  FunctionElement,
  #[serde(rename = "local function")]
  LocalFunctionElement,
  #[serde(rename = "method")]
  MemberFunctionElement,
  #[serde(rename = "getter")]
  MemberGetAccessorElement,
  #[serde(rename = "setter")]
  MemberSetAccessorElement,
  #[serde(rename = "property")]
  MemberVariableElement,
  #[serde(rename = "constructor")]
  ConstructorImplementationElement,
  #[serde(rename = "call")]
  CallSignatureElement,
  #[serde(rename = "index")]
  IndexSignatureElement,
  #[serde(rename = "construct")]
  ConstructSignatureElement,
  #[serde(rename = "parameter")]
  ParameterElement,
  #[serde(rename = "type parameter")]
  TypeParameterElement,
  #[serde(rename = "primitive type")]
  PrimitiveType,
  #[serde(rename = "label")]
  Label,
  #[serde(rename = "alias")]
  Alias,
  #[serde(rename = "const")]
  ConstElement,
  #[serde(rename = "let")]
  LetElement,
  #[serde(rename = "directory")]
  Directory,
  #[serde(rename = "external module name")]
  ExternalModuleName,
  #[serde(rename = "JSX attribute")]
  JsxAttribute,
  #[serde(rename = "string")]
  String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSpan {
  start: u32,
  length: u32,
}

impl TextSpan {
  pub fn to_range(&self, line_index: &[u32]) -> lsp_types::Range {
    lsp_types::Range {
      start: text::to_position(line_index, self.start),
      end: text::to_position(line_index, self.start + self.length),
    }
  }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SymbolDisplayPart {
  text: String,
  kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JSDocTagInfo {
  name: String,
  text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickInfo {
  kind: ScriptElementKind,
  kind_modifiers: String,
  text_span: TextSpan,
  display_parts: Option<Vec<SymbolDisplayPart>>,
  documentation: Option<Vec<SymbolDisplayPart>>,
  tags: Option<Vec<JSDocTagInfo>>,
}

impl QuickInfo {
  pub fn to_hover(&self, line_index: &[u32]) -> lsp_types::Hover {
    let mut contents = Vec::<lsp_types::MarkedString>::new();
    if let Some(display_string) =
      display_parts_to_string(self.display_parts.clone())
    {
      contents.push(lsp_types::MarkedString::from_language_code(
        "typescript".to_string(),
        display_string,
      ));
    }
    if let Some(documentation) =
      display_parts_to_string(self.documentation.clone())
    {
      contents.push(lsp_types::MarkedString::from_markdown(documentation));
    }
    if let Some(tags) = &self.tags {
      let tags_preview = tags
        .iter()
        .map(get_tag_documentation)
        .collect::<Vec<String>>()
        .join("  \n\n");
      if !tags_preview.is_empty() {
        contents.push(lsp_types::MarkedString::from_markdown(format!(
          "\n\n{}",
          tags_preview
        )));
      }
    }
    lsp_types::Hover {
      contents: lsp_types::HoverContents::Array(contents),
      range: Some(self.text_span.to_range(line_index)),
    }
  }
}

#[derive(Debug, Deserialize)]
pub enum HighlightSpanKind {
  #[serde(rename = "none")]
  None,
  #[serde(rename = "definition")]
  Definition,
  #[serde(rename = "reference")]
  Reference,
  #[serde(rename = "writtenReference")]
  WrittenReference,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightSpan {
  file_name: Option<String>,
  is_in_string: Option<bool>,
  text_span: TextSpan,
  context_span: Option<TextSpan>,
  kind: HighlightSpanKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHighlights {
  file_name: String,
  highlight_spans: Vec<HighlightSpan>,
}

impl DocumentHighlights {
  pub fn to_highlight(
    &self,
    line_index: &[u32],
  ) -> Vec<lsp_types::DocumentHighlight> {
    self
      .highlight_spans
      .iter()
      .map(|hs| lsp_types::DocumentHighlight {
        range: hs.text_span.to_range(line_index),
        kind: match hs.kind {
          HighlightSpanKind::WrittenReference => {
            Some(lsp_types::DocumentHighlightKind::Write)
          }
          _ => Some(lsp_types::DocumentHighlightKind::Read),
        },
      })
      .collect()
  }
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
  id: usize,
  data: Value,
}

struct State<'a> {
  last_id: usize,
  response: Option<Response>,
  server_state: ServerStateSnapshot,
  snapshots: HashMap<(Cow<'a, str>, Cow<'a, str>), String>,
}

impl<'a> State<'a> {
  fn new(server_state: ServerStateSnapshot) -> Self {
    Self {
      last_id: 1,
      response: None,
      server_state,
      snapshots: Default::default(),
    }
  }
}

/// If a snapshot is missing from the state cache, add it.
fn cache_snapshot(
  state: &mut State,
  specifier: String,
  version: String,
) -> Result<(), AnyError> {
  if !state
    .snapshots
    .contains_key(&(specifier.clone().into(), version.clone().into()))
  {
    let s = ModuleSpecifier::resolve_url(&specifier)?;
    let file_cache = state.server_state.file_cache.read().unwrap();
    let file_id = file_cache.lookup(&s).unwrap();
    let content = file_cache.get_contents(file_id)?;
    state
      .snapshots
      .insert((specifier.into(), version.into()), content);
  }
  Ok(())
}

fn op<F>(op_fn: F) -> Box<OpFn>
where
  F: Fn(&mut State, Value) -> Result<Value, AnyError> + 'static,
{
  json_op_sync(move |s, args, _bufs| {
    let state = s.borrow_mut::<State>();
    op_fn(state, args)
  })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceSnapshotArgs {
  specifier: String,
  version: String,
}

/// The language service is dropping a reference to a source file snapshot, and
/// we can drop our version of that document.
fn dispose(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: SourceSnapshotArgs = serde_json::from_value(args)?;
  state
    .snapshots
    .remove(&(v.specifier.into(), v.version.into()));
  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetChangeRangeArgs {
  specifier: String,
  old_version: String,
  version: String,
}

/// The language service wants to compare an old snapshot with a new snapshot to
/// determine what source hash changed.
fn get_change_range(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: GetChangeRangeArgs = serde_json::from_value(args.clone())?;
  cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
  if let (Some(a), Some(b)) = (
    state
      .snapshots
      .get(&(v.specifier.clone().into(), v.old_version.clone().into())),
    state
      .snapshots
      .get(&(v.specifier.clone().into(), v.version.into())),
  ) {
    Ok(text::get_range_change(a, b))
  } else {
    Err(custom_error(
      "MissingSnapshot",
      format!(
        "One of the snapshotted versions is missing.\n  Args: \"{}\"",
        args
      ),
    ))
  }
}

fn get_length(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: SourceSnapshotArgs = serde_json::from_value(args)?;
  let specifier = ModuleSpecifier::resolve_url(&v.specifier)?;
  if state.server_state.doc_data.contains_key(&specifier) {
    cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
    let content = state
      .snapshots
      .get(&(v.specifier.into(), v.version.into()))
      .unwrap();
    Ok(json!(content.chars().count()))
  } else {
    let mut sources = state.server_state.sources.write().unwrap();
    Ok(json!(sources.get_length(&specifier).unwrap()))
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetTextArgs {
  specifier: String,
  version: String,
  start: usize,
  end: usize,
}

fn get_text(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: GetTextArgs = serde_json::from_value(args)?;
  let specifier = ModuleSpecifier::resolve_url(&v.specifier)?;
  let content = if state.server_state.doc_data.contains_key(&specifier) {
    cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
    state
      .snapshots
      .get(&(v.specifier.into(), v.version.into()))
      .unwrap()
      .clone()
  } else {
    let mut sources = state.server_state.sources.write().unwrap();
    sources.get_text(&specifier).unwrap()
  };
  Ok(json!(text::slice(&content, v.start..v.end)))
}

fn resolve(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: ResolveArgs = serde_json::from_value(args)?;
  let mut resolved = Vec::<Option<(String, String)>>::new();
  let referrer = ModuleSpecifier::resolve_url(&v.base)?;
  let mut sources = if let Ok(sources) = state.server_state.sources.write() {
    sources
  } else {
    return Err(custom_error("Deadlock", "deadlock locking sources"));
  };

  if let Some(doc_data) = state.server_state.doc_data.get(&referrer) {
    if let Some(dependencies) = &doc_data.dependencies {
      for specifier in &v.specifiers {
        if specifier.starts_with("asset:///") {
          resolved.push(Some((
            specifier.clone(),
            MediaType::from(specifier).as_ts_extension(),
          )))
        } else if let Some(dependency) = dependencies.get(specifier) {
          let resolved_import =
            if let Some(resolved_import) = &dependency.maybe_type {
              resolved_import.clone()
            } else if let Some(resolved_import) = &dependency.maybe_code {
              resolved_import.clone()
            } else {
              ResolvedImport::Err("missing dependency".to_string())
            };
          if let ResolvedImport::Resolved(resolved_specifier) = resolved_import
          {
            let media_type = if let Some(media_type) =
              sources.get_media_type(&resolved_specifier)
            {
              media_type
            } else {
              MediaType::from(&resolved_specifier)
            };
            resolved.push(Some((
              resolved_specifier.to_string(),
              media_type.as_ts_extension(),
            )));
          } else {
            resolved.push(None);
          }
        }
      }
    }
  } else if sources.contains(&referrer) {
    for specifier in &v.specifiers {
      if let Some((resolved_specifier, media_type)) =
        sources.resolve_import(specifier, &referrer)
      {
        resolved.push(Some((
          resolved_specifier.to_string(),
          media_type.as_ts_extension(),
        )));
      } else {
        resolved.push(None);
      }
    }
  } else {
    return Err(custom_error(
      "NotFound",
      "the referring specifier is unexpectedly missing",
    ));
  }

  Ok(json!(resolved))
}

fn respond(state: &mut State, args: Value) -> Result<Value, AnyError> {
  state.response = Some(serde_json::from_value(args)?);
  Ok(json!(true))
}

fn script_names(state: &mut State, _args: Value) -> Result<Value, AnyError> {
  let script_names: Vec<&ModuleSpecifier> =
    state.server_state.doc_data.keys().collect();
  Ok(json!(script_names))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptVersionArgs {
  specifier: String,
}

fn script_version(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: ScriptVersionArgs = serde_json::from_value(args)?;
  let specifier = ModuleSpecifier::resolve_url(&v.specifier)?;
  let maybe_doc_data = state.server_state.doc_data.get(&specifier);
  if let Some(doc_data) = maybe_doc_data {
    if let Some(version) = doc_data.version {
      return Ok(json!(version.to_string()));
    }
  } else {
    let mut sources = state.server_state.sources.write().unwrap();
    if let Some(version) = sources.get_script_version(&specifier) {
      return Ok(json!(version));
    }
  }

  Ok(json!(None::<String>))
}

/// Create and setup a JsRuntime based on a snapshot. It is expected that the
/// supplied snapshot is an isolate that contains the TypeScript language
/// server.
pub fn start(snapshot: Snapshot, debug: bool) -> Result<JsRuntime, AnyError> {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });

  {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    op_state.put(State::new(ServerStateSnapshot::default()));
  }

  runtime.register_op("op_dispose", op(dispose));
  runtime.register_op("op_get_change_range", op(get_change_range));
  runtime.register_op("op_get_length", op(get_length));
  runtime.register_op("op_get_text", op(get_text));
  runtime.register_op("op_resolve", op(resolve));
  runtime.register_op("op_respond", op(respond));
  runtime.register_op("op_script_names", op(script_names));
  runtime.register_op("op_script_version", op(script_version));

  let init_config = json!({ "debug": debug });
  let init_src = format!("globalThis.serverInit({});", init_config);

  runtime.execute("[native code]", &init_src)?;
  Ok(runtime)
}

/// Methods that are supported by the Language Service in the compiler isolate.
pub enum RequestMethod {
  /// Configure the compilation settings for the server.
  Configure(TsConfig),
  /// Return semantic diagnostics for given file.
  GetSemanticDiagnostics(ModuleSpecifier),
  /// Returns suggestion diagnostics for given file.
  GetSuggestionDiagnostics(ModuleSpecifier),
  /// Return syntactic diagnostics for a given file.
  GetSyntacticDiagnostics(ModuleSpecifier),
  /// Return quick info at position (hover information).
  GetQuickInfo((ModuleSpecifier, u32)),
  /// Return document highlights at position.
  GetDocumentHighlights((ModuleSpecifier, u32, Vec<ModuleSpecifier>)),
}

impl RequestMethod {
  pub fn to_value(&self, id: usize) -> Value {
    match self {
      RequestMethod::Configure(config) => json!({
        "id": id,
        "method": "configure",
        "compilerOptions": config,
      }),
      RequestMethod::GetSemanticDiagnostics(specifier) => json!({
        "id": id,
        "method": "getSemanticDiagnostics",
        "specifier": specifier,
      }),
      RequestMethod::GetSuggestionDiagnostics(specifier) => json!({
        "id": id,
        "method": "getSuggestionDiagnostics",
        "specifier": specifier,
      }),
      RequestMethod::GetSyntacticDiagnostics(specifier) => json!({
        "id": id,
        "method": "getSyntacticDiagnostics",
        "specifier": specifier,
      }),
      RequestMethod::GetQuickInfo((specifier, position)) => json!({
        "id": id,
        "method": "getQuickInfo",
        "specifier": specifier,
        "position": position,
      }),
      RequestMethod::GetDocumentHighlights((
        specifier,
        position,
        files_to_search,
      )) => json!({
        "id": id,
        "method": "getDocumentHighlights",
        "specifier": specifier,
        "position": position,
        "filesToSearch": files_to_search,
      }),
    }
  }
}

/// Send a request into a runtime and return the JSON value of the response.
pub fn request(
  runtime: &mut JsRuntime,
  server_state: &ServerStateSnapshot,
  method: RequestMethod,
) -> Result<Value, AnyError> {
  let id = {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    let state = op_state.borrow_mut::<State>();
    state.server_state = server_state.clone();
    state.last_id += 1;
    state.last_id
  };
  let request_params = method.to_value(id);
  let request_src = format!("globalThis.serverRequest({});", request_params);
  runtime.execute("[native_code]", &request_src)?;

  let op_state = runtime.op_state();
  let mut op_state = op_state.borrow_mut();
  let state = op_state.borrow_mut::<State>();

  if let Some(response) = state.response.clone() {
    state.response = None;
    Ok(response.data)
  } else {
    Err(custom_error(
      "RequestError",
      "The response was not received for the request.",
    ))
  }
}

#[cfg(test)]
mod tests {
  use super::super::memory_cache::MemoryCache;
  use super::super::state::DocumentData;
  use super::*;
  use crate::js;
  use std::collections::HashMap;
  use std::sync::Arc;
  use std::sync::RwLock;

  fn mock_server_state(sources: Vec<(&str, &str, i32)>) -> ServerStateSnapshot {
    let mut doc_data = HashMap::new();
    let mut file_cache = MemoryCache::default();
    for (specifier, content, version) in sources {
      let specifier = ModuleSpecifier::resolve_url(specifier)
        .expect("failed to create specifier");
      doc_data.insert(
        specifier.clone(),
        DocumentData::new(specifier.clone(), version, content, None),
      );
      file_cache.set_contents(specifier, Some(content.as_bytes().to_vec()));
    }
    let file_cache = Arc::new(RwLock::new(file_cache));
    ServerStateSnapshot {
      config: Default::default(),
      diagnostics: Default::default(),
      doc_data,
      file_cache,
      sources: Default::default(),
    }
  }

  fn setup(
    debug: bool,
    config: Value,
    sources: Vec<(&str, &str, i32)>,
  ) -> (JsRuntime, ServerStateSnapshot) {
    let server_state = mock_server_state(sources.clone());
    let mut runtime = start(js::compiler_isolate_init(), debug)
      .expect("could not start server");
    let ts_config = TsConfig::new(config);
    assert_eq!(
      request(
        &mut runtime,
        &server_state,
        RequestMethod::Configure(ts_config)
      )
      .expect("failed request"),
      json!(true)
    );
    (runtime, server_state)
  }

  #[test]
  fn test_replace_links() {
    let actual = replace_links(r"test {@link http://deno.land/x/mod.ts} test");
    assert_eq!(
      actual,
      r"test [http://deno.land/x/mod.ts](http://deno.land/x/mod.ts) test"
    );
    let actual =
      replace_links(r"test {@link http://deno.land/x/mod.ts a link} test");
    assert_eq!(actual, r"test [a link](http://deno.land/x/mod.ts) test");
    let actual =
      replace_links(r"test {@linkcode http://deno.land/x/mod.ts a link} test");
    assert_eq!(actual, r"test [`a link`](http://deno.land/x/mod.ts) test");
  }

  #[test]
  fn test_project_configure() {
    setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      vec![],
    );
  }

  #[test]
  fn test_project_reconfigure() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      vec![],
    );
    let ts_config = TsConfig::new(json!({
      "target": "esnext",
      "module": "esnext",
      "noEmit": true,
      "lib": ["deno.ns", "deno.worker"]
    }));
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::Configure(ts_config),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!(true));
  }

  #[test]
  fn test_get_semantic_diagnostics() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      vec![("file:///a.ts", r#"console.log("hello deno");"#, 1)],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::GetSemanticDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!([
        {
          "start": {
            "line": 0,
            "character": 0,
          },
          "end": {
            "line": 0,
            "character": 7
          },
          "fileName": "file:///a.ts",
          "messageText": "Cannot find name 'console'. Do you need to change your target library? Try changing the `lib` compiler option to include 'dom'.",
          "sourceLine": "console.log(\"hello deno\");",
          "category": 1,
          "code": 2584
        }
      ])
    );
  }

  #[test]
  fn test_module_resolution() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      vec![(
        "file:///a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
      )],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::GetSemanticDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!([]));
  }

  #[test]
  fn test_bad_module_specifiers() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      vec![(
        "file:///a.ts",
        r#"
        import { A } from ".";
        "#,
        1,
      )],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::GetSyntacticDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!([]));
  }

  #[test]
  fn test_remote_modules() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      vec![(
        "file:///a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
      )],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::GetSyntacticDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!([]));
  }
}
