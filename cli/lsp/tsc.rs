// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::analysis::ResolvedDependency;
use super::language_server::StateSnapshot;
use super::text;
use super::utils;

use crate::media_type::MediaType;
use crate::tokio_util::create_basic_runtime;
use crate::tsc;
use crate::tsc::ResolveArgs;
use crate::tsc_config::TsConfig;

use deno_core::error::anyhow;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::Future;
use deno_core::json_op_sync;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpFn;
use deno_core::RuntimeOptions;
use lspower::lsp_types;
use regex::Captures;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

type Request = (
  RequestMethod,
  StateSnapshot,
  oneshot::Sender<Result<Value, AnyError>>,
);

#[derive(Clone, Debug)]
pub struct TsServer(mpsc::UnboundedSender<Request>);

impl TsServer {
  pub fn new() -> Self {
    let (tx, mut rx) = mpsc::unbounded_channel::<Request>();
    let _join_handle = thread::spawn(move || {
      // TODO(@kitsonk) we need to allow displaying diagnostics here, but the
      // current compiler snapshot sends them to stdio which would totally break
      // the language server...
      let mut ts_runtime = start(false).expect("could not start tsc");

      let mut runtime = create_basic_runtime();
      runtime.block_on(async {
        while let Some((req, state_snapshot, tx)) = rx.recv().await {
          let value = request(&mut ts_runtime, state_snapshot, req);
          if tx.send(value).is_err() {
            warn!("Unable to send result to client.");
          }
        }
      })
    });

    Self(tx)
  }

  pub async fn request(
    &self,
    snapshot: StateSnapshot,
    req: RequestMethod,
  ) -> Result<Value, AnyError> {
    let (tx, rx) = oneshot::channel::<Result<Value, AnyError>>();
    if self.0.send((req, snapshot, tx)).is_err() {
      return Err(anyhow!("failed to send request to tsc thread"));
    }
    rx.await?
  }
}

/// Optionally returns an internal asset, first checking for any static assets
/// in Rust, then checking any previously retrieved static assets from the
/// isolate, and then finally, the tsc isolate itself.
pub async fn get_asset(
  specifier: &ModuleSpecifier,
  ts_server: &TsServer,
  state_snapshot: &StateSnapshot,
) -> Result<Option<String>, AnyError> {
  let specifier_str = specifier.to_string().replace("asset:///", "");
  if let Some(asset_text) = tsc::get_asset(&specifier_str) {
    Ok(Some(asset_text.to_string()))
  } else {
    {
      let assets = state_snapshot.assets.lock().unwrap();
      if let Some(asset) = assets.get(specifier) {
        return Ok(asset.clone());
      }
    }
    let asset: Option<String> = serde_json::from_value(
      ts_server
        .request(
          state_snapshot.clone(),
          RequestMethod::GetAsset(specifier.clone()),
        )
        .await?,
    )?;
    let mut assets = state_snapshot.assets.lock().unwrap();
    assets.insert(specifier.clone(), asset.clone());
    Ok(asset)
  }
}

fn display_parts_to_string(
  maybe_parts: Option<Vec<SymbolDisplayPart>>,
) -> Option<String> {
  maybe_parts.map(|parts| {
    parts
      .into_iter()
      .map(|p| p.text)
      .collect::<Vec<String>>()
      .join("")
  })
}

fn get_tag_body_text(tag: &JSDocTagInfo) -> Option<String> {
  tag.text.as_ref().map(|text| match tag.name.as_str() {
    "example" => {
      let caption_regex =
        Regex::new(r"<caption>(.*?)</caption>\s*\r?\n((?:\s|\S)*)").unwrap();
      if caption_regex.is_match(&text) {
        caption_regex
          .replace(text, |c: &Captures| {
            format!("{}\n\n{}", &c[1], make_codeblock(&c[2]))
          })
          .to_string()
      } else {
        make_codeblock(text)
      }
    }
    "author" => {
      let email_match_regex = Regex::new(r"(.+)\s<([-.\w]+@[-.\w]+)>").unwrap();
      email_match_regex
        .replace(text, |c: &Captures| format!("{} {}", &c[1], &c[2]))
        .to_string()
    }
    "default" => make_codeblock(text),
    _ => replace_links(text),
  })
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

#[derive(Debug, Clone, Deserialize)]
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

impl From<ScriptElementKind> for lsp_types::CompletionItemKind {
  fn from(kind: ScriptElementKind) -> Self {
    use lspower::lsp_types::CompletionItemKind;

    match kind {
      ScriptElementKind::PrimitiveType | ScriptElementKind::Keyword => {
        CompletionItemKind::Keyword
      }
      ScriptElementKind::ConstElement => CompletionItemKind::Constant,
      ScriptElementKind::LetElement
      | ScriptElementKind::VariableElement
      | ScriptElementKind::LocalVariableElement
      | ScriptElementKind::Alias => CompletionItemKind::Variable,
      ScriptElementKind::MemberVariableElement
      | ScriptElementKind::MemberGetAccessorElement
      | ScriptElementKind::MemberSetAccessorElement => {
        CompletionItemKind::Field
      }
      ScriptElementKind::FunctionElement => CompletionItemKind::Function,
      ScriptElementKind::MemberFunctionElement
      | ScriptElementKind::ConstructSignatureElement
      | ScriptElementKind::CallSignatureElement
      | ScriptElementKind::IndexSignatureElement => CompletionItemKind::Method,
      ScriptElementKind::EnumElement => CompletionItemKind::Enum,
      ScriptElementKind::ModuleElement
      | ScriptElementKind::ExternalModuleName => CompletionItemKind::Module,
      ScriptElementKind::ClassElement | ScriptElementKind::TypeElement => {
        CompletionItemKind::Class
      }
      ScriptElementKind::InterfaceElement => CompletionItemKind::Interface,
      ScriptElementKind::Warning | ScriptElementKind::ScriptElement => {
        CompletionItemKind::File
      }
      ScriptElementKind::Directory => CompletionItemKind::Folder,
      ScriptElementKind::String => CompletionItemKind::Constant,
      _ => CompletionItemKind::Property,
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
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
#[serde(rename_all = "camelCase")]
pub struct DocumentSpan {
  text_span: TextSpan,
  pub file_name: String,
  original_text_span: Option<TextSpan>,
  original_file_name: Option<String>,
  context_span: Option<TextSpan>,
  original_context_span: Option<TextSpan>,
}

impl DocumentSpan {
  pub async fn to_link<F, Fut>(
    &self,
    line_index: &[u32],
    index_provider: F,
  ) -> Option<lsp_types::LocationLink>
  where
    F: Fn(ModuleSpecifier) -> Fut,
    Fut: Future<Output = Result<Vec<u32>, AnyError>>,
  {
    let target_specifier =
      ModuleSpecifier::resolve_url(&self.file_name).unwrap();
    if let Ok(target_line_index) = index_provider(target_specifier).await {
      let target_uri = utils::normalize_file_name(&self.file_name).unwrap();
      let (target_range, target_selection_range) =
        if let Some(context_span) = &self.context_span {
          (
            context_span.to_range(&target_line_index),
            self.text_span.to_range(&target_line_index),
          )
        } else {
          (
            self.text_span.to_range(&target_line_index),
            self.text_span.to_range(&target_line_index),
          )
        };
      let link = lsp_types::LocationLink {
        origin_selection_range: Some(self.text_span.to_range(line_index)),
        target_uri,
        target_range,
        target_selection_range,
      };
      Some(link)
    } else {
      None
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationLocation {
  #[serde(flatten)]
  pub document_span: DocumentSpan,
  // ImplementationLocation props
  kind: ScriptElementKind,
  display_parts: Vec<SymbolDisplayPart>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameLocation {
  // inherit from DocumentSpan
  text_span: TextSpan,
  file_name: String,
  original_text_span: Option<TextSpan>,
  original_file_name: Option<String>,
  context_span: Option<TextSpan>,
  original_context_span: Option<TextSpan>,
  // RenameLocation props
  prefix_text: Option<String>,
  suffix_text: Option<String>,
}

pub struct RenameLocations {
  pub locations: Vec<RenameLocation>,
}

impl RenameLocations {
  pub async fn into_workspace_edit<F, Fut>(
    self,
    snapshot: StateSnapshot,
    index_provider: F,
    new_name: &str,
  ) -> Result<lsp_types::WorkspaceEdit, AnyError>
  where
    F: Fn(ModuleSpecifier) -> Fut,
    Fut: Future<Output = Result<Vec<u32>, AnyError>>,
  {
    let mut text_document_edit_map: HashMap<Url, lsp_types::TextDocumentEdit> =
      HashMap::new();
    for location in self.locations.iter() {
      let uri = utils::normalize_file_name(&location.file_name)?;
      let specifier = ModuleSpecifier::resolve_url(&location.file_name)?;

      // ensure TextDocumentEdit for `location.file_name`.
      if text_document_edit_map.get(&uri).is_none() {
        text_document_edit_map.insert(
          uri.clone(),
          lsp_types::TextDocumentEdit {
            text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
              uri: uri.clone(),
              version: snapshot
                .doc_data
                .get(&specifier)
                .map_or_else(|| None, |data| data.version),
            },
            edits: Vec::<
              lsp_types::OneOf<
                lsp_types::TextEdit,
                lsp_types::AnnotatedTextEdit,
              >,
            >::new(),
          },
        );
      }

      // push TextEdit for ensured `TextDocumentEdit.edits`.
      let document_edit = text_document_edit_map.get_mut(&uri).unwrap();
      document_edit
        .edits
        .push(lsp_types::OneOf::Left(lsp_types::TextEdit {
          range: location
            .text_span
            .to_range(&index_provider(specifier.clone()).await?),
          new_text: new_name.to_string(),
        }));
    }

    Ok(lsp_types::WorkspaceEdit {
      changes: None,
      document_changes: Some(lsp_types::DocumentChanges::Edits(
        text_document_edit_map.values().cloned().collect(),
      )),
    })
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
pub struct DefinitionInfo {
  kind: ScriptElementKind,
  name: String,
  container_kind: Option<ScriptElementKind>,
  container_name: Option<String>,

  #[serde(flatten)]
  pub document_span: DocumentSpan,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionInfoAndBoundSpan {
  pub definitions: Option<Vec<DefinitionInfo>>,
  text_span: TextSpan,
}

impl DefinitionInfoAndBoundSpan {
  pub async fn to_definition<F, Fut>(
    &self,
    line_index: &[u32],
    index_provider: F,
  ) -> Option<lsp_types::GotoDefinitionResponse>
  where
    F: Fn(ModuleSpecifier) -> Fut + Clone,
    Fut: Future<Output = Result<Vec<u32>, AnyError>>,
  {
    if let Some(definitions) = &self.definitions {
      let mut location_links = Vec::<lsp_types::LocationLink>::new();
      for di in definitions {
        if let Some(link) = di.document_span.to_link(line_index, index_provider.clone()).await {
          location_links.push(link);
        }
      }
      Some(lsp_types::GotoDefinitionResponse::Link(location_links))
    } else {
      None
    }
  }
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceEntry {
  is_write_access: bool,
  pub is_definition: bool,
  is_in_string: Option<bool>,
  text_span: TextSpan,
  pub file_name: String,
  original_text_span: Option<TextSpan>,
  original_file_name: Option<String>,
  context_span: Option<TextSpan>,
  original_context_span: Option<TextSpan>,
}

impl ReferenceEntry {
  pub fn to_location(&self, line_index: &[u32]) -> lsp_types::Location {
    let uri = utils::normalize_file_name(&self.file_name).unwrap();
    lsp_types::Location {
      uri,
      range: self.text_span.to_range(line_index),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionInfo {
  entries: Vec<CompletionEntry>,
  is_member_completion: bool,
}

impl CompletionInfo {
  pub fn into_completion_response(
    self,
    line_index: &[u32],
  ) -> lsp_types::CompletionResponse {
    let items = self
      .entries
      .into_iter()
      .map(|entry| entry.into_completion_item(line_index))
      .collect();
    lsp_types::CompletionResponse::Array(items)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionEntry {
  kind: ScriptElementKind,
  kind_modifiers: Option<String>,
  name: String,
  sort_text: String,
  insert_text: Option<String>,
  replacement_span: Option<TextSpan>,
  has_action: Option<bool>,
  source: Option<String>,
  is_recommended: Option<bool>,
}

impl CompletionEntry {
  pub fn into_completion_item(
    self,
    line_index: &[u32],
  ) -> lsp_types::CompletionItem {
    let mut item = lsp_types::CompletionItem {
      label: self.name,
      kind: Some(self.kind.into()),
      sort_text: Some(self.sort_text.clone()),
      // TODO(lucacasonato): missing commit_characters
      ..Default::default()
    };

    if let Some(true) = self.is_recommended {
      // Make sure isRecommended property always comes first
      // https://github.com/Microsoft/vscode/issues/40325
      item.preselect = Some(true);
    } else if self.source.is_some() {
      // De-prioritze auto-imports
      // https://github.com/Microsoft/vscode/issues/40311
      item.sort_text = Some("\u{ffff}".to_string() + &self.sort_text)
    }

    match item.kind {
      Some(lsp_types::CompletionItemKind::Function)
      | Some(lsp_types::CompletionItemKind::Method) => {
        item.insert_text_format = Some(lsp_types::InsertTextFormat::Snippet);
      }
      _ => {}
    }

    let mut insert_text = self.insert_text;
    let replacement_range: Option<lsp_types::Range> =
      self.replacement_span.map(|span| span.to_range(line_index));

    // TODO(lucacasonato): port other special cases from https://github.com/theia-ide/typescript-language-server/blob/fdf28313833cd6216d00eb4e04dc7f00f4c04f09/server/src/completion.ts#L49-L55

    if let Some(kind_modifiers) = self.kind_modifiers {
      if kind_modifiers.contains("\\optional\\") {
        if insert_text.is_none() {
          insert_text = Some(item.label.clone());
        }
        if item.filter_text.is_none() {
          item.filter_text = Some(item.label.clone());
        }
        item.label += "?";
      }
    }

    if let Some(insert_text) = insert_text {
      if let Some(replacement_range) = replacement_range {
        item.text_edit = Some(lsp_types::CompletionTextEdit::Edit(
          lsp_types::TextEdit::new(replacement_range, insert_text),
        ));
      } else {
        item.insert_text = Some(insert_text);
      }
    }

    item
  }
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
  id: usize,
  data: Value,
}

struct State<'a> {
  asset: Option<String>,
  last_id: usize,
  response: Option<Response>,
  state_snapshot: StateSnapshot,
  snapshots: HashMap<(Cow<'a, str>, Cow<'a, str>), String>,
}

impl<'a> State<'a> {
  fn new(state_snapshot: StateSnapshot) -> Self {
    Self {
      asset: None,
      last_id: 1,
      response: None,
      state_snapshot,
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
    let content = {
      let file_cache = state.state_snapshot.file_cache.lock().unwrap();
      let file_id = file_cache.lookup(&s).unwrap();
      file_cache.get_contents(file_id)?
    };
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
  old_length: u32,
  old_version: String,
  version: String,
}

/// The language service wants to compare an old snapshot with a new snapshot to
/// determine what source hash changed.
fn get_change_range(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: GetChangeRangeArgs = serde_json::from_value(args.clone())?;
  cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
  if let Some(current) = state
    .snapshots
    .get(&(v.specifier.clone().into(), v.version.into()))
  {
    if let Some(prev) = state
      .snapshots
      .get(&(v.specifier.clone().into(), v.old_version.clone().into()))
    {
      Ok(text::get_range_change(prev, current))
    } else {
      // when a local file is opened up in the editor, the compiler might
      // already have a snapshot of it in memory, and will request it, but we
      // now are working off in memory versions of the document, and so need
      // to tell tsc to reset the whole document
      Ok(json!({
        "span": {
          "start": 0,
          "length": v.old_length,
        },
        "newLength": current.chars().count(),
      }))
    }
  } else {
    Err(custom_error(
      "MissingSnapshot",
      format!(
        "The current snapshot version is missing.\n  Args: \"{}\"",
        args
      ),
    ))
  }
}

fn get_length(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: SourceSnapshotArgs = serde_json::from_value(args)?;
  let specifier = ModuleSpecifier::resolve_url(&v.specifier)?;
  if state.state_snapshot.doc_data.contains_key(&specifier) {
    cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
    let content = state
      .snapshots
      .get(&(v.specifier.into(), v.version.into()))
      .unwrap();
    Ok(json!(content.chars().count()))
  } else {
    let mut sources = state.state_snapshot.sources.lock().unwrap();
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
  let content = if state.state_snapshot.doc_data.contains_key(&specifier) {
    cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
    state
      .snapshots
      .get(&(v.specifier.into(), v.version.into()))
      .unwrap()
      .clone()
  } else {
    let mut sources = state.state_snapshot.sources.lock().unwrap();
    sources.get_text(&specifier).unwrap()
  };
  Ok(json!(text::slice(&content, v.start..v.end)))
}

fn resolve(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: ResolveArgs = serde_json::from_value(args)?;
  let mut resolved = Vec::<Option<(String, String)>>::new();
  let referrer = ModuleSpecifier::resolve_url(&v.base)?;
  let mut sources = if let Ok(sources) = state.state_snapshot.sources.lock() {
    sources
  } else {
    return Err(custom_error("Deadlock", "deadlock locking sources"));
  };

  if let Some(doc_data) = state.state_snapshot.doc_data.get(&referrer) {
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
              ResolvedDependency::Err("missing dependency".to_string())
            };
          if let ResolvedDependency::Resolved(resolved_specifier) =
            resolved_import
          {
            if state
              .state_snapshot
              .doc_data
              .contains_key(&resolved_specifier)
              || sources.contains(&resolved_specifier)
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
    state.state_snapshot.doc_data.keys().collect();
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
  let maybe_doc_data = state.state_snapshot.doc_data.get(&specifier);
  if let Some(doc_data) = maybe_doc_data {
    if let Some(version) = doc_data.version {
      return Ok(json!(version.to_string()));
    }
  } else {
    let mut sources = state.state_snapshot.sources.lock().unwrap();
    if let Some(version) = sources.get_script_version(&specifier) {
      return Ok(json!(version));
    }
  }

  Ok(json!(None::<String>))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetAssetArgs {
  text: Option<String>,
}

fn set_asset(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: SetAssetArgs = serde_json::from_value(args)?;
  state.asset = v.text;
  Ok(json!(true))
}

/// Create and setup a JsRuntime based on a snapshot. It is expected that the
/// supplied snapshot is an isolate that contains the TypeScript language
/// server.
pub fn start(debug: bool) -> Result<JsRuntime, AnyError> {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(tsc::compiler_snapshot()),
    ..Default::default()
  });

  {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    op_state.put(State::new(StateSnapshot::default()));
  }

  runtime.register_op("op_dispose", op(dispose));
  runtime.register_op("op_get_change_range", op(get_change_range));
  runtime.register_op("op_get_length", op(get_length));
  runtime.register_op("op_get_text", op(get_text));
  runtime.register_op("op_resolve", op(resolve));
  runtime.register_op("op_respond", op(respond));
  runtime.register_op("op_script_names", op(script_names));
  runtime.register_op("op_script_version", op(script_version));
  runtime.register_op("op_set_asset", op(set_asset));

  let init_config = json!({ "debug": debug });
  let init_src = format!("globalThis.serverInit({});", init_config);

  runtime.execute("[native code]", &init_src)?;
  Ok(runtime)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum QuotePreference {
  Auto,
  Double,
  Single,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum ImportModuleSpecifierPreference {
  Auto,
  Relative,
  NonRelative,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum ImportModuleSpecifierEnding {
  Auto,
  Minimal,
  Index,
  Js,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum IncludePackageJsonAutoImports {
  Auto,
  On,
  Off,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub disable_suggestions: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub quote_preference: Option<QuotePreference>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_for_module_exports: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_automatic_optional_chain_completions: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_insert_text: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub import_module_specifier_preference:
    Option<ImportModuleSpecifierPreference>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub import_module_specifier_ending: Option<ImportModuleSpecifierEnding>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub allow_text_changes_in_new_files: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provide_prefix_and_suffix_text_for_rename: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_package_json_auto_imports: Option<IncludePackageJsonAutoImports>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provide_refactor_not_applicable_reason: Option<bool>,
}

/// Methods that are supported by the Language Service in the compiler isolate.
pub enum RequestMethod {
  /// Configure the compilation settings for the server.
  Configure(TsConfig),
  /// Retrieve the text of an assets that exists in memory in the isolate.
  GetAsset(ModuleSpecifier),
  /// Return diagnostics for given file.
  GetDiagnostics(ModuleSpecifier),
  /// Return quick info at position (hover information).
  GetQuickInfo((ModuleSpecifier, u32)),
  /// Return document highlights at position.
  GetDocumentHighlights((ModuleSpecifier, u32, Vec<ModuleSpecifier>)),
  /// Get document references for a specific position.
  GetReferences((ModuleSpecifier, u32)),
  /// Get declaration information for a specific position.
  GetDefinition((ModuleSpecifier, u32)),
  /// Get completion information at a given position (IntelliSense).
  GetCompletions((ModuleSpecifier, u32, UserPreferences)),
  /// Get implementation information for a specific position.
  GetImplementation((ModuleSpecifier, u32)),
  /// Get rename locations at a given position.
  FindRenameLocations((ModuleSpecifier, u32, bool, bool, bool)),
}

impl RequestMethod {
  pub fn to_value(&self, id: usize) -> Value {
    match self {
      RequestMethod::Configure(config) => json!({
        "id": id,
        "method": "configure",
        "compilerOptions": config,
      }),
      RequestMethod::GetAsset(specifier) => json!({
        "id": id,
        "method": "getAsset",
        "specifier": specifier,
      }),
      RequestMethod::GetDiagnostics(specifier) => json!({
        "id": id,
        "method": "getDiagnostics",
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
      RequestMethod::GetReferences((specifier, position)) => json!({
        "id": id,
        "method": "getReferences",
        "specifier": specifier,
        "position": position,
      }),
      RequestMethod::GetDefinition((specifier, position)) => json!({
        "id": id,
        "method": "getDefinition",
        "specifier": specifier,
        "position": position,
      }),
      RequestMethod::GetCompletions((specifier, position, preferences)) => {
        json!({
          "id": id,
          "method": "getCompletions",
          "specifier": specifier,
          "position": position,
          "preferences": preferences,
        })
      }
      RequestMethod::GetImplementation((specifier, position)) => json!({
          "id": id,
          "method": "getImplementation",
          "specifier": specifier,
          "position": position,
      }),
      RequestMethod::FindRenameLocations((
        specifier,
        position,
        find_in_strings,
        find_in_comments,
        provide_prefix_and_suffix_text_for_rename,
      )) => {
        json!({
          "id": id,
          "method": "findRenameLocations",
          "specifier": specifier,
          "position": position,
          "findInStrings": find_in_strings,
          "findInComments": find_in_comments,
          "providePrefixAndSuffixTextForRename": provide_prefix_and_suffix_text_for_rename
        })
      }
    }
  }
}

/// Send a request into a runtime and return the JSON value of the response.
pub fn request(
  runtime: &mut JsRuntime,
  state_snapshot: StateSnapshot,
  method: RequestMethod,
) -> Result<Value, AnyError> {
  let id = {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    let state = op_state.borrow_mut::<State>();
    state.state_snapshot = state_snapshot;
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
  use super::*;
  use crate::lsp::language_server::DocumentData;
  use std::collections::HashMap;
  use std::sync::Arc;
  use std::sync::Mutex;

  fn mock_state_snapshot(sources: Vec<(&str, &str, i32)>) -> StateSnapshot {
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
    let file_cache = Arc::new(Mutex::new(file_cache));
    StateSnapshot {
      assets: Default::default(),
      doc_data,
      file_cache,
      sources: Default::default(),
    }
  }

  fn setup(
    debug: bool,
    config: Value,
    sources: Vec<(&str, &str, i32)>,
  ) -> (JsRuntime, StateSnapshot) {
    let state_snapshot = mock_state_snapshot(sources.clone());
    let mut runtime = start(debug).expect("could not start server");
    let ts_config = TsConfig::new(config);
    assert_eq!(
      request(
        &mut runtime,
        state_snapshot.clone(),
        RequestMethod::Configure(ts_config)
      )
      .expect("failed request"),
      json!(true)
    );
    (runtime, state_snapshot)
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
    let (mut runtime, state_snapshot) = setup(
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
      state_snapshot,
      RequestMethod::Configure(ts_config),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!(true));
  }

  #[test]
  fn test_get_diagnostics() {
    let (mut runtime, state_snapshot) = setup(
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
      state_snapshot,
      RequestMethod::GetDiagnostics(specifier),
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
    let (mut runtime, state_snapshot) = setup(
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
      state_snapshot,
      RequestMethod::GetDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!([]));
  }

  #[test]
  fn test_bad_module_specifiers() {
    let (mut runtime, state_snapshot) = setup(
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
      state_snapshot,
      RequestMethod::GetDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!([{
        "start": {
          "line": 1,
          "character": 8
        },
        "end": {
          "line": 1,
          "character": 30
        },
        "fileName": "file:///a.ts",
        "messageText": "\'A\' is declared but its value is never read.",
        "sourceLine": "        import { A } from \".\";",
        "category": 2,
        "code": 6133,
        "reportsUnnecessary": true,
      }])
    );
  }

  #[test]
  fn test_remote_modules() {
    let (mut runtime, state_snapshot) = setup(
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
      state_snapshot,
      RequestMethod::GetDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!([]));
  }

  #[test]
  fn test_partial_modules() {
    let (mut runtime, state_snapshot) = setup(
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
        import {
          Application,
          Context,
          Router,
          Status,
        } from "https://deno.land/x/oak@v6.3.2/mod.ts";
        
        import * as test from
      "#,
        1,
      )],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!([{
        "start": {
          "line": 1,
          "character": 8
        },
        "end": {
          "line": 6,
          "character": 55,
        },
        "fileName": "file:///a.ts",
        "messageText": "All imports in import declaration are unused.",
        "sourceLine": "        import {",
        "category": 2,
        "code": 6192,
        "reportsUnnecessary": true
      }, {
        "start": {
          "line": 8,
          "character": 29
        },
        "end": {
          "line": 8,
          "character": 29
        },
        "fileName": "file:///a.ts",
        "messageText": "Expression expected.",
        "sourceLine": "        import * as test from",
        "category": 1,
        "code": 1109
      }])
    );
  }

  #[test]
  fn test_no_debug_failure() {
    let (mut runtime, state_snapshot) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      vec![("file:///a.ts", r#"const url = new URL("b.js", import."#, 1)],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!([]));
  }

  #[test]
  fn test_request_asset() {
    let (mut runtime, state_snapshot) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      vec![],
    );
    let specifier = ModuleSpecifier::resolve_url("asset:///lib.esnext.d.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetAsset(specifier),
    );
    assert!(result.is_ok());
    let response: Option<String> =
      serde_json::from_value(result.unwrap()).unwrap();
    assert!(response.is_some());
  }
}
