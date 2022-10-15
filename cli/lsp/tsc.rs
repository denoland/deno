// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::code_lens;
use super::completions::relative_specifier;
use super::config;
use super::documents::AssetOrDocument;
use super::language_server;
use super::language_server::StateSnapshot;
use super::performance::Performance;
use super::refactor::RefactorCodeActionData;
use super::refactor::ALL_KNOWN_REFACTOR_ACTION_KINDS;
use super::refactor::EXTRACT_CONSTANT;
use super::refactor::EXTRACT_INTERFACE;
use super::refactor::EXTRACT_TYPE;
use super::semantic_tokens;
use super::semantic_tokens::SemanticTokensBuilder;
use super::text::LineIndex;
use super::urls::LspUrlMap;
use super::urls::INVALID_SPECIFIER;

use crate::args::TsConfig;
use crate::fs_util::specifier_to_file_path;
use crate::tsc;
use crate::tsc::ResolveArgs;

use deno_core::anyhow::anyhow;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::op;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url;
use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::RuntimeOptions;
use deno_runtime::tokio_util::create_basic_runtime;
use log::warn;
use once_cell::sync::Lazy;
use regex::Captures;
use regex::Regex;
use serde_repr::Deserialize_repr;
use serde_repr::Serialize_repr;
use std::cmp;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use text_size::TextRange;
use text_size::TextSize;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tower_lsp::jsonrpc::Error as LspError;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types as lsp;

static BRACKET_ACCESSOR_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"^\[['"](.+)[\['"]\]$"#).unwrap());
static CAPTION_RE: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r"<caption>(.*?)</caption>\s*\r?\n((?:\s|\S)*)").unwrap()
});
static CODEBLOCK_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^\s*[~`]{3}").unwrap());
static EMAIL_MATCH_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"(.+)\s<([-.\w]+@[-.\w]+)>").unwrap());
static HTTP_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"(?i)^https?:"#).unwrap());
static JSDOC_LINKS_RE: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r"(?i)\{@(link|linkplain|linkcode) (https?://[^ |}]+?)(?:[| ]([^{}\n]+?))?\}").unwrap()
});
static PART_KIND_MODIFIER_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r",|\s+").unwrap());
static PART_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^(\S+)\s*-?\s*").unwrap());
static SCOPE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"scope_(\d)").unwrap());

const FILE_EXTENSION_KIND_MODIFIERS: &[&str] =
  &[".d.ts", ".ts", ".tsx", ".js", ".jsx", ".json"];

type Request = (
  RequestMethod,
  Arc<StateSnapshot>,
  oneshot::Sender<Result<Value, AnyError>>,
  CancellationToken,
);

#[derive(Clone, Debug)]
pub struct TsServer(mpsc::UnboundedSender<Request>);

impl TsServer {
  pub fn new(performance: Arc<Performance>) -> Self {
    let (tx, mut rx) = mpsc::unbounded_channel::<Request>();
    let _join_handle = thread::spawn(move || {
      let mut ts_runtime = js_runtime(performance);

      let runtime = create_basic_runtime();
      runtime.block_on(async {
        let mut started = false;
        while let Some((req, state_snapshot, tx, token)) = rx.recv().await {
          if !started {
            // TODO(@kitsonk) need to reflect the debug state of the lsp here
            start(&mut ts_runtime, false, &state_snapshot)
              .expect("could not start tsc");
            started = true;
          }
          let value = request(&mut ts_runtime, state_snapshot, req, token);
          if tx.send(value).is_err() {
            warn!("Unable to send result to client.");
          }
        }
      })
    });

    Self(tx)
  }

  pub async fn request<R>(
    &self,
    snapshot: Arc<StateSnapshot>,
    req: RequestMethod,
  ) -> Result<R, AnyError>
  where
    R: de::DeserializeOwned,
  {
    self
      .request_with_cancellation(snapshot, req, Default::default())
      .await
  }

  pub async fn request_with_cancellation<R>(
    &self,
    snapshot: Arc<StateSnapshot>,
    req: RequestMethod,
    token: CancellationToken,
  ) -> Result<R, AnyError>
  where
    R: de::DeserializeOwned,
  {
    let (tx, rx) = oneshot::channel::<Result<Value, AnyError>>();
    if self.0.send((req, snapshot, tx, token)).is_err() {
      return Err(anyhow!("failed to send request to tsc thread"));
    }
    rx.await?.map(|v| serde_json::from_value::<R>(v).unwrap())
  }
}

#[derive(Debug, Clone)]
struct AssetDocumentInner {
  specifier: ModuleSpecifier,
  text: Arc<str>,
  line_index: Arc<LineIndex>,
  maybe_navigation_tree: Option<Arc<NavigationTree>>,
}

/// An lsp representation of an asset in memory, that has either been retrieved
/// from static assets built into Rust, or static assets built into tsc.
#[derive(Debug, Clone)]
pub struct AssetDocument(Arc<AssetDocumentInner>);

impl AssetDocument {
  pub fn new(specifier: ModuleSpecifier, text: impl AsRef<str>) -> Self {
    let text = text.as_ref();
    Self(Arc::new(AssetDocumentInner {
      specifier,
      text: text.into(),
      line_index: Arc::new(LineIndex::new(text)),
      maybe_navigation_tree: None,
    }))
  }

  pub fn specifier(&self) -> &ModuleSpecifier {
    &self.0.specifier
  }

  pub fn with_navigation_tree(
    &self,
    tree: Arc<NavigationTree>,
  ) -> AssetDocument {
    AssetDocument(Arc::new(AssetDocumentInner {
      maybe_navigation_tree: Some(tree),
      ..(*self.0).clone()
    }))
  }

  pub fn text(&self) -> Arc<str> {
    self.0.text.clone()
  }

  pub fn line_index(&self) -> Arc<LineIndex> {
    self.0.line_index.clone()
  }

  pub fn maybe_navigation_tree(&self) -> Option<Arc<NavigationTree>> {
    self.0.maybe_navigation_tree.clone()
  }
}

type AssetsMap = HashMap<ModuleSpecifier, AssetDocument>;

fn new_assets_map() -> Arc<Mutex<AssetsMap>> {
  let assets = tsc::STATIC_ASSETS
    .iter()
    .map(|(k, v)| {
      let url_str = format!("asset:///{}", k);
      let specifier = resolve_url(&url_str).unwrap();
      let asset = AssetDocument::new(specifier.clone(), v);
      (specifier, asset)
    })
    .collect();
  Arc::new(Mutex::new(assets))
}

/// Snapshot of Assets.
#[derive(Debug, Clone)]
pub struct AssetsSnapshot(Arc<Mutex<AssetsMap>>);

impl Default for AssetsSnapshot {
  fn default() -> Self {
    Self(new_assets_map())
  }
}

impl AssetsSnapshot {
  pub fn contains_key(&self, k: &ModuleSpecifier) -> bool {
    self.0.lock().contains_key(k)
  }

  pub fn get(&self, k: &ModuleSpecifier) -> Option<AssetDocument> {
    self.0.lock().get(k).cloned()
  }
}

/// Assets are never updated and so we can safely use this struct across
/// multiple threads without needing to worry about race conditions.
#[derive(Debug, Clone)]
pub struct Assets {
  ts_server: Arc<TsServer>,
  assets: Arc<Mutex<AssetsMap>>,
}

impl Assets {
  pub fn new(ts_server: Arc<TsServer>) -> Self {
    Self {
      ts_server,
      assets: new_assets_map(),
    }
  }

  /// Initializes with the assets in the isolate.
  pub async fn intitialize(&self, state_snapshot: Arc<StateSnapshot>) {
    let assets = get_isolate_assets(&self.ts_server, state_snapshot).await;
    let mut assets_map = self.assets.lock();
    for asset in assets {
      if !assets_map.contains_key(asset.specifier()) {
        assets_map.insert(asset.specifier().clone(), asset);
      }
    }
  }

  pub fn snapshot(&self) -> AssetsSnapshot {
    // it's ok to not make a complete copy for snapshotting purposes
    // because assets are static
    AssetsSnapshot(self.assets.clone())
  }

  pub fn get(&self, specifier: &ModuleSpecifier) -> Option<AssetDocument> {
    self.assets.lock().get(specifier).cloned()
  }

  pub fn cache_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
    navigation_tree: Arc<NavigationTree>,
  ) -> Result<(), AnyError> {
    let mut assets = self.assets.lock();
    let doc = assets
      .get_mut(specifier)
      .ok_or_else(|| anyhow!("Missing asset."))?;
    *doc = doc.with_navigation_tree(navigation_tree);
    Ok(())
  }
}

/// Get all the assets stored in the tsc isolate.
async fn get_isolate_assets(
  ts_server: &TsServer,
  state_snapshot: Arc<StateSnapshot>,
) -> Vec<AssetDocument> {
  let res: Value = ts_server
    .request(state_snapshot, RequestMethod::GetAssets)
    .await
    .unwrap();
  let response_assets = match res {
    Value::Array(value) => value,
    _ => unreachable!(),
  };
  let mut assets = Vec::with_capacity(response_assets.len());

  for asset in response_assets {
    let mut obj = match asset {
      Value::Object(obj) => obj,
      _ => unreachable!(),
    };
    let specifier_str = obj.get("specifier").unwrap().as_str().unwrap();
    let specifier = ModuleSpecifier::parse(specifier_str).unwrap();
    let text = match obj.remove("text").unwrap() {
      Value::String(text) => text,
      _ => unreachable!(),
    };
    assets.push(AssetDocument::new(specifier, text));
  }

  assets
}

fn get_tag_body_text(
  tag: &JsDocTagInfo,
  language_server: &language_server::Inner,
) -> Option<String> {
  tag.text.as_ref().map(|display_parts| {
    // TODO(@kitsonk) check logic in vscode about handling this API change in
    // tsserver
    let text = display_parts_to_string(display_parts, language_server);
    match tag.name.as_str() {
      "example" => {
        if CAPTION_RE.is_match(&text) {
          CAPTION_RE
            .replace(&text, |c: &Captures| {
              format!("{}\n\n{}", &c[1], make_codeblock(&c[2]))
            })
            .to_string()
        } else {
          make_codeblock(&text)
        }
      }
      "author" => EMAIL_MATCH_RE
        .replace(&text, |c: &Captures| format!("{} {}", &c[1], &c[2]))
        .to_string(),
      "default" => make_codeblock(&text),
      _ => replace_links(&text),
    }
  })
}

fn get_tag_documentation(
  tag: &JsDocTagInfo,
  language_server: &language_server::Inner,
) -> String {
  match tag.name.as_str() {
    "augments" | "extends" | "param" | "template" => {
      if let Some(display_parts) = &tag.text {
        // TODO(@kitsonk) check logic in vscode about handling this API change
        // in tsserver
        let text = display_parts_to_string(display_parts, language_server);
        let body: Vec<&str> = PART_RE.split(&text).collect();
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
  let maybe_text = get_tag_body_text(tag, language_server);
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
  if CODEBLOCK_RE.is_match(text) {
    text.to_string()
  } else {
    format!("```\n{}\n```", text)
  }
}

/// Replace JSDoc like links (`{@link http://example.com}`) with markdown links
fn replace_links<S: AsRef<str>>(text: S) -> String {
  JSDOC_LINKS_RE
    .replace_all(text.as_ref(), |c: &Captures| match &c[1] {
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

fn parse_kind_modifier(kind_modifiers: &str) -> HashSet<&str> {
  PART_KIND_MODIFIER_RE.split(kind_modifiers).collect()
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
  One(T),
  Many(Vec<T>),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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
  #[serde(rename = "link")]
  Link,
  #[serde(rename = "link name")]
  LinkName,
  #[serde(rename = "link text")]
  LinkText,
}

impl Default for ScriptElementKind {
  fn default() -> Self {
    Self::Unknown
  }
}

/// This mirrors the method `convertKind` in `completions.ts` in vscode
impl From<ScriptElementKind> for lsp::CompletionItemKind {
  fn from(kind: ScriptElementKind) -> Self {
    match kind {
      ScriptElementKind::PrimitiveType | ScriptElementKind::Keyword => {
        lsp::CompletionItemKind::KEYWORD
      }
      ScriptElementKind::ConstElement
      | ScriptElementKind::LetElement
      | ScriptElementKind::VariableElement
      | ScriptElementKind::LocalVariableElement
      | ScriptElementKind::Alias
      | ScriptElementKind::ParameterElement => {
        lsp::CompletionItemKind::VARIABLE
      }
      ScriptElementKind::MemberVariableElement
      | ScriptElementKind::MemberGetAccessorElement
      | ScriptElementKind::MemberSetAccessorElement => {
        lsp::CompletionItemKind::FIELD
      }
      ScriptElementKind::FunctionElement
      | ScriptElementKind::LocalFunctionElement => {
        lsp::CompletionItemKind::FUNCTION
      }
      ScriptElementKind::MemberFunctionElement
      | ScriptElementKind::ConstructSignatureElement
      | ScriptElementKind::CallSignatureElement
      | ScriptElementKind::IndexSignatureElement => {
        lsp::CompletionItemKind::METHOD
      }
      ScriptElementKind::EnumElement => lsp::CompletionItemKind::ENUM,
      ScriptElementKind::EnumMemberElement => {
        lsp::CompletionItemKind::ENUM_MEMBER
      }
      ScriptElementKind::ModuleElement
      | ScriptElementKind::ExternalModuleName => {
        lsp::CompletionItemKind::MODULE
      }
      ScriptElementKind::ClassElement | ScriptElementKind::TypeElement => {
        lsp::CompletionItemKind::CLASS
      }
      ScriptElementKind::InterfaceElement => lsp::CompletionItemKind::INTERFACE,
      ScriptElementKind::Warning => lsp::CompletionItemKind::TEXT,
      ScriptElementKind::ScriptElement => lsp::CompletionItemKind::FILE,
      ScriptElementKind::Directory => lsp::CompletionItemKind::FOLDER,
      ScriptElementKind::String => lsp::CompletionItemKind::CONSTANT,
      _ => lsp::CompletionItemKind::PROPERTY,
    }
  }
}

/// This mirrors `fromProtocolScriptElementKind` in vscode
impl From<ScriptElementKind> for lsp::SymbolKind {
  fn from(kind: ScriptElementKind) -> Self {
    match kind {
      ScriptElementKind::ModuleElement => Self::MODULE,
      // this is only present in `getSymbolKind` in `workspaceSymbols` in
      // vscode, but seems strange it isn't consistent.
      ScriptElementKind::TypeElement => Self::CLASS,
      ScriptElementKind::ClassElement => Self::CLASS,
      ScriptElementKind::EnumElement => Self::ENUM,
      ScriptElementKind::EnumMemberElement => Self::ENUM_MEMBER,
      ScriptElementKind::InterfaceElement => Self::INTERFACE,
      ScriptElementKind::IndexSignatureElement => Self::METHOD,
      ScriptElementKind::CallSignatureElement => Self::METHOD,
      ScriptElementKind::MemberFunctionElement => Self::METHOD,
      // workspaceSymbols in vscode treats them as fields, which does seem more
      // semantically correct while `fromProtocolScriptElementKind` treats them
      // as properties.
      ScriptElementKind::MemberVariableElement => Self::FIELD,
      ScriptElementKind::MemberGetAccessorElement => Self::FIELD,
      ScriptElementKind::MemberSetAccessorElement => Self::FIELD,
      ScriptElementKind::VariableElement => Self::VARIABLE,
      ScriptElementKind::LetElement => Self::VARIABLE,
      ScriptElementKind::ConstElement => Self::VARIABLE,
      ScriptElementKind::LocalVariableElement => Self::VARIABLE,
      ScriptElementKind::Alias => Self::VARIABLE,
      ScriptElementKind::FunctionElement => Self::FUNCTION,
      ScriptElementKind::LocalFunctionElement => Self::FUNCTION,
      ScriptElementKind::ConstructSignatureElement => Self::CONSTRUCTOR,
      ScriptElementKind::ConstructorImplementationElement => Self::CONSTRUCTOR,
      ScriptElementKind::TypeParameterElement => Self::TYPE_PARAMETER,
      ScriptElementKind::String => Self::STRING,
      _ => Self::VARIABLE,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TextSpan {
  pub start: u32,
  pub length: u32,
}

impl TextSpan {
  pub fn from_range(
    range: &lsp::Range,
    line_index: Arc<LineIndex>,
  ) -> Result<Self, AnyError> {
    let start = line_index.offset_tsc(range.start)?;
    let length = line_index.offset_tsc(range.end)? - start;
    Ok(Self { start, length })
  }

  pub fn to_range(&self, line_index: Arc<LineIndex>) -> lsp::Range {
    lsp::Range {
      start: line_index.position_tsc(self.start.into()),
      end: line_index.position_tsc(TextSize::from(self.start + self.length)),
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SymbolDisplayPart {
  text: String,
  kind: String,
  // This is only on `JSDocLinkDisplayPart` which extends `SymbolDisplayPart`
  // but is only used as an upcast of a `SymbolDisplayPart` and not explicitly
  // returned by any API, so it is safe to add it as an optional value.
  target: Option<DocumentSpan>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsDocTagInfo {
  name: String,
  text: Option<Vec<SymbolDisplayPart>>,
}

// Note: the tsc protocol contains fields that are part of the protocol but
// not currently used.  They are commented out in the structures so it is clear
// that they exist.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickInfo {
  // kind: ScriptElementKind,
  // kind_modifiers: String,
  text_span: TextSpan,
  display_parts: Option<Vec<SymbolDisplayPart>>,
  documentation: Option<Vec<SymbolDisplayPart>>,
  tags: Option<Vec<JsDocTagInfo>>,
}

#[derive(Default)]
struct Link {
  name: Option<String>,
  target: Option<DocumentSpan>,
  text: Option<String>,
  linkcode: bool,
}

/// Takes `SymbolDisplayPart` items and converts them into a string, handling
/// any `{@link Symbol}` and `{@linkcode Symbol}` JSDoc tags and linking them
/// to the their source location.
fn display_parts_to_string(
  parts: &[SymbolDisplayPart],
  language_server: &language_server::Inner,
) -> String {
  let mut out = Vec::<String>::new();

  let mut current_link: Option<Link> = None;
  for part in parts {
    match part.kind.as_str() {
      "link" => {
        if let Some(link) = current_link.as_mut() {
          if let Some(target) = &link.target {
            if let Some(specifier) = target.to_target(language_server) {
              let link_text = link.text.clone().unwrap_or_else(|| {
                link
                  .name
                  .clone()
                  .map(|ref n| n.replace('`', "\\`"))
                  .unwrap_or_else(|| "".to_string())
              });
              let link_str = if link.linkcode {
                format!("[`{}`]({})", link_text, specifier)
              } else {
                format!("[{}]({})", link_text, specifier)
              };
              out.push(link_str);
            }
          } else {
            let maybe_text = link.text.clone().or_else(|| link.name.clone());
            if let Some(text) = maybe_text {
              if HTTP_RE.is_match(&text) {
                let parts: Vec<&str> = text.split(' ').collect();
                if parts.len() == 1 {
                  out.push(parts[0].to_string());
                } else {
                  let link_text = parts[1..].join(" ").replace('`', "\\`");
                  let link_str = if link.linkcode {
                    format!("[`{}`]({})", link_text, parts[0])
                  } else {
                    format!("[{}]({})", link_text, parts[0])
                  };
                  out.push(link_str);
                }
              } else {
                out.push(text.replace('`', "\\`"));
              }
            }
          }
          current_link = None;
        } else {
          current_link = Some(Link {
            linkcode: part.text.as_str() == "{@linkcode ",
            ..Default::default()
          });
        }
      }
      "linkName" => {
        if let Some(link) = current_link.as_mut() {
          link.name = Some(part.text.clone());
          link.target = part.target.clone();
        }
      }
      "linkText" => {
        if let Some(link) = current_link.as_mut() {
          link.name = Some(part.text.clone());
        }
      }
      _ => out.push(part.text.clone()),
    }
  }

  replace_links(out.join(""))
}

impl QuickInfo {
  pub fn to_hover(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> lsp::Hover {
    let mut parts = Vec::<lsp::MarkedString>::new();
    if let Some(display_string) = self
      .display_parts
      .clone()
      .map(|p| display_parts_to_string(&p, language_server))
    {
      parts.push(lsp::MarkedString::from_language_code(
        "typescript".to_string(),
        display_string,
      ));
    }
    if let Some(documentation) = self
      .documentation
      .clone()
      .map(|p| display_parts_to_string(&p, language_server))
    {
      parts.push(lsp::MarkedString::from_markdown(documentation));
    }
    if let Some(tags) = &self.tags {
      let tags_preview = tags
        .iter()
        .map(|tag_info| get_tag_documentation(tag_info, language_server))
        .collect::<Vec<String>>()
        .join("  \n\n");
      if !tags_preview.is_empty() {
        parts.push(lsp::MarkedString::from_markdown(format!(
          "\n\n{}",
          tags_preview
        )));
      }
    }
    lsp::Hover {
      contents: lsp::HoverContents::Array(parts),
      range: Some(self.text_span.to_range(line_index)),
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSpan {
  text_span: TextSpan,
  pub file_name: String,
  original_text_span: Option<TextSpan>,
  // original_file_name: Option<String>,
  context_span: Option<TextSpan>,
  original_context_span: Option<TextSpan>,
}

impl DocumentSpan {
  pub fn to_link(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> Option<lsp::LocationLink> {
    let target_specifier = normalize_specifier(&self.file_name).ok()?;
    let target_asset_or_doc =
      language_server.get_maybe_asset_or_document(&target_specifier)?;
    let target_line_index = target_asset_or_doc.line_index();
    let target_uri = language_server
      .url_map
      .normalize_specifier(&target_specifier)
      .ok()?;
    let (target_range, target_selection_range) =
      if let Some(context_span) = &self.context_span {
        (
          context_span.to_range(target_line_index.clone()),
          self.text_span.to_range(target_line_index),
        )
      } else {
        (
          self.text_span.to_range(target_line_index.clone()),
          self.text_span.to_range(target_line_index),
        )
      };
    let origin_selection_range =
      if let Some(original_context_span) = &self.original_context_span {
        Some(original_context_span.to_range(line_index))
      } else {
        self
          .original_text_span
          .as_ref()
          .map(|original_text_span| original_text_span.to_range(line_index))
      };
    let link = lsp::LocationLink {
      origin_selection_range,
      target_uri,
      target_range,
      target_selection_range,
    };
    Some(link)
  }

  /// Convert the `DocumentSpan` into a specifier that can be sent to the client
  /// to link to the target document span. Used for converting JSDoc symbol
  /// links to markdown links.
  fn to_target(
    &self,
    language_server: &language_server::Inner,
  ) -> Option<ModuleSpecifier> {
    let specifier = normalize_specifier(&self.file_name).ok()?;
    let asset_or_doc =
      language_server.get_maybe_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let range = self.text_span.to_range(line_index);
    let mut target = language_server
      .url_map
      .normalize_specifier(&specifier)
      .ok()?;
    target.set_fragment(Some(&format!(
      "L{},{}",
      range.start.line + 1,
      range.start.character + 1
    )));

    Some(target)
  }
}

#[derive(Debug, Clone, Deserialize)]
pub enum MatchKind {
  #[serde(rename = "exact")]
  Exact,
  #[serde(rename = "prefix")]
  Prefix,
  #[serde(rename = "substring")]
  Substring,
  #[serde(rename = "camelCase")]
  CamelCase,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateToItem {
  name: String,
  kind: ScriptElementKind,
  kind_modifiers: String,
  // match_kind: MatchKind,
  // is_case_sensitive: bool,
  file_name: String,
  text_span: TextSpan,
  container_name: Option<String>,
  // container_kind: ScriptElementKind,
}

impl NavigateToItem {
  pub fn to_symbol_information(
    &self,
    language_server: &mut language_server::Inner,
  ) -> Option<lsp::SymbolInformation> {
    let specifier = normalize_specifier(&self.file_name).ok()?;
    let asset_or_doc =
      language_server.get_asset_or_document(&specifier).ok()?;
    let line_index = asset_or_doc.line_index();
    let uri = language_server
      .url_map
      .normalize_specifier(&specifier)
      .ok()?;
    let range = self.text_span.to_range(line_index);
    let location = lsp::Location { uri, range };

    let mut tags: Option<Vec<lsp::SymbolTag>> = None;
    let kind_modifiers = parse_kind_modifier(&self.kind_modifiers);
    if kind_modifiers.contains("deprecated") {
      tags = Some(vec![lsp::SymbolTag::DEPRECATED]);
    }

    // The field `deprecated` is deprecated but SymbolInformation does not have
    // a default, therefore we have to supply the deprecated deprecated
    // field. It is like a bad version of Inception.
    #[allow(deprecated)]
    Some(lsp::SymbolInformation {
      name: self.name.clone(),
      kind: self.kind.clone().into(),
      tags,
      deprecated: None,
      location,
      container_name: self.container_name.clone(),
    })
  }
}

#[derive(Debug, Clone, Deserialize)]
pub enum InlayHintKind {
  Type,
  Parameter,
  Enum,
}

impl InlayHintKind {
  pub fn to_lsp(&self) -> Option<lsp::InlayHintKind> {
    match self {
      Self::Enum => None,
      Self::Parameter => Some(lsp::InlayHintKind::PARAMETER),
      Self::Type => Some(lsp::InlayHintKind::TYPE),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlayHint {
  pub text: String,
  pub position: u32,
  pub kind: InlayHintKind,
  pub whitespace_before: Option<bool>,
  pub whitespace_after: Option<bool>,
}

impl InlayHint {
  pub fn to_lsp(&self, line_index: Arc<LineIndex>) -> lsp::InlayHint {
    lsp::InlayHint {
      position: line_index.position_tsc(self.position.into()),
      label: lsp::InlayHintLabel::String(self.text.clone()),
      kind: self.kind.to_lsp(),
      padding_left: self.whitespace_before,
      padding_right: self.whitespace_after,
      text_edits: None,
      tooltip: None,
      data: None,
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationTree {
  pub text: String,
  pub kind: ScriptElementKind,
  pub kind_modifiers: String,
  pub spans: Vec<TextSpan>,
  pub name_span: Option<TextSpan>,
  pub child_items: Option<Vec<NavigationTree>>,
}

impl NavigationTree {
  pub fn to_code_lens(
    &self,
    line_index: Arc<LineIndex>,
    specifier: &ModuleSpecifier,
    source: &code_lens::CodeLensSource,
  ) -> lsp::CodeLens {
    let range = if let Some(name_span) = &self.name_span {
      name_span.to_range(line_index)
    } else if !self.spans.is_empty() {
      let span = &self.spans[0];
      span.to_range(line_index)
    } else {
      lsp::Range::default()
    };
    lsp::CodeLens {
      range,
      command: None,
      data: Some(json!({
        "specifier": specifier,
        "source": source
      })),
    }
  }

  pub fn collect_document_symbols(
    &self,
    line_index: Arc<LineIndex>,
    document_symbols: &mut Vec<lsp::DocumentSymbol>,
  ) -> bool {
    let mut should_include = self.should_include_entry();
    if !should_include
      && self.child_items.as_ref().map_or(true, |v| v.is_empty())
    {
      return false;
    }

    let children = self
      .child_items
      .as_ref()
      .map_or(&[] as &[NavigationTree], |v| v.as_slice());
    for span in self.spans.iter() {
      let range = TextRange::at(span.start.into(), span.length.into());
      let mut symbol_children = Vec::<lsp::DocumentSymbol>::new();
      for child in children.iter() {
        let should_traverse_child = child
          .spans
          .iter()
          .map(|child_span| {
            TextRange::at(child_span.start.into(), child_span.length.into())
          })
          .any(|child_range| range.intersect(child_range).is_some());
        if should_traverse_child {
          let included_child = child
            .collect_document_symbols(line_index.clone(), &mut symbol_children);
          should_include = should_include || included_child;
        }
      }

      if should_include {
        let mut selection_span = span;
        if let Some(name_span) = self.name_span.as_ref() {
          let name_range =
            TextRange::at(name_span.start.into(), name_span.length.into());
          if range.contains_range(name_range) {
            selection_span = name_span;
          }
        }

        let name = match self.kind {
          ScriptElementKind::MemberGetAccessorElement => {
            format!("(get) {}", self.text)
          }
          ScriptElementKind::MemberSetAccessorElement => {
            format!("(set) {}", self.text)
          }
          _ => self.text.clone(),
        };

        let mut tags: Option<Vec<lsp::SymbolTag>> = None;
        let kind_modifiers = parse_kind_modifier(&self.kind_modifiers);
        if kind_modifiers.contains("deprecated") {
          tags = Some(vec![lsp::SymbolTag::DEPRECATED]);
        }

        let children = if !symbol_children.is_empty() {
          Some(symbol_children)
        } else {
          None
        };

        // The field `deprecated` is deprecated but DocumentSymbol does not have
        // a default, therefore we have to supply the deprecated deprecated
        // field. It is like a bad version of Inception.
        #[allow(deprecated)]
        document_symbols.push(lsp::DocumentSymbol {
          name,
          kind: self.kind.clone().into(),
          range: span.to_range(line_index.clone()),
          selection_range: selection_span.to_range(line_index.clone()),
          tags,
          children,
          detail: None,
          deprecated: None,
        })
      }
    }

    should_include
  }

  fn should_include_entry(&self) -> bool {
    if let ScriptElementKind::Alias = self.kind {
      return false;
    }

    !self.text.is_empty() && self.text != "<function>" && self.text != "<class>"
  }

  pub fn walk<F>(&self, callback: &F)
  where
    F: Fn(&NavigationTree, Option<&NavigationTree>),
  {
    callback(self, None);
    if let Some(child_items) = &self.child_items {
      for child in child_items {
        child.walk_child(callback, self);
      }
    }
  }

  fn walk_child<F>(&self, callback: &F, parent: &NavigationTree)
  where
    F: Fn(&NavigationTree, Option<&NavigationTree>),
  {
    callback(self, Some(parent));
    if let Some(child_items) = &self.child_items {
      for child in child_items {
        child.walk_child(callback, self);
      }
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationLocation {
  #[serde(flatten)]
  pub document_span: DocumentSpan,
  // ImplementationLocation props
  // kind: ScriptElementKind,
  // display_parts: Vec<SymbolDisplayPart>,
}

impl ImplementationLocation {
  pub fn to_location(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> lsp::Location {
    let specifier = normalize_specifier(&self.document_span.file_name)
      .unwrap_or_else(|_| ModuleSpecifier::parse("deno://invalid").unwrap());
    let uri = language_server
      .url_map
      .normalize_specifier(&specifier)
      .unwrap_or_else(|_| ModuleSpecifier::parse("deno://invalid").unwrap());
    lsp::Location {
      uri,
      range: self.document_span.text_span.to_range(line_index),
    }
  }

  pub fn to_link(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> Option<lsp::LocationLink> {
    self.document_span.to_link(line_index, language_server)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameLocation {
  #[serde(flatten)]
  document_span: DocumentSpan,
  // RenameLocation props
  // prefix_text: Option<String>,
  // suffix_text: Option<String>,
}

pub struct RenameLocations {
  pub locations: Vec<RenameLocation>,
}

impl RenameLocations {
  pub async fn into_workspace_edit(
    self,
    new_name: &str,
    language_server: &language_server::Inner,
  ) -> Result<lsp::WorkspaceEdit, AnyError> {
    let mut text_document_edit_map: HashMap<Url, lsp::TextDocumentEdit> =
      HashMap::new();
    for location in self.locations.iter() {
      let specifier = normalize_specifier(&location.document_span.file_name)?;
      let uri = language_server.url_map.normalize_specifier(&specifier)?;
      let asset_or_doc = language_server.get_asset_or_document(&specifier)?;

      // ensure TextDocumentEdit for `location.file_name`.
      if text_document_edit_map.get(&uri).is_none() {
        text_document_edit_map.insert(
          uri.clone(),
          lsp::TextDocumentEdit {
            text_document: lsp::OptionalVersionedTextDocumentIdentifier {
              uri: uri.clone(),
              version: asset_or_doc.document_lsp_version(),
            },
            edits:
              Vec::<lsp::OneOf<lsp::TextEdit, lsp::AnnotatedTextEdit>>::new(),
          },
        );
      }

      // push TextEdit for ensured `TextDocumentEdit.edits`.
      let document_edit = text_document_edit_map.get_mut(&uri).unwrap();
      document_edit.edits.push(lsp::OneOf::Left(lsp::TextEdit {
        range: location
          .document_span
          .text_span
          .to_range(asset_or_doc.line_index()),
        new_text: new_name.to_string(),
      }));
    }

    Ok(lsp::WorkspaceEdit {
      change_annotations: None,
      changes: None,
      document_changes: Some(lsp::DocumentChanges::Edits(
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
  // file_name: Option<String>,
  // is_in_string: Option<bool>,
  text_span: TextSpan,
  // context_span: Option<TextSpan>,
  kind: HighlightSpanKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionInfo {
  // kind: ScriptElementKind,
  // name: String,
  // container_kind: Option<ScriptElementKind>,
  // container_name: Option<String>,
  #[serde(flatten)]
  pub document_span: DocumentSpan,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionInfoAndBoundSpan {
  pub definitions: Option<Vec<DefinitionInfo>>,
  // text_span: TextSpan,
}

impl DefinitionInfoAndBoundSpan {
  pub async fn to_definition(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> Option<lsp::GotoDefinitionResponse> {
    if let Some(definitions) = &self.definitions {
      let mut location_links = Vec::<lsp::LocationLink>::new();
      for di in definitions {
        if let Some(link) = di
          .document_span
          .to_link(line_index.clone(), language_server)
        {
          location_links.push(link);
        }
      }
      Some(lsp::GotoDefinitionResponse::Link(location_links))
    } else {
      None
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHighlights {
  // file_name: String,
  highlight_spans: Vec<HighlightSpan>,
}

impl DocumentHighlights {
  pub fn to_highlight(
    &self,
    line_index: Arc<LineIndex>,
  ) -> Vec<lsp::DocumentHighlight> {
    self
      .highlight_spans
      .iter()
      .map(|hs| lsp::DocumentHighlight {
        range: hs.text_span.to_range(line_index.clone()),
        kind: match hs.kind {
          HighlightSpanKind::WrittenReference => {
            Some(lsp::DocumentHighlightKind::WRITE)
          }
          _ => Some(lsp::DocumentHighlightKind::READ),
        },
      })
      .collect()
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TextChange {
  pub span: TextSpan,
  pub new_text: String,
}

impl TextChange {
  pub fn as_text_edit(&self, line_index: Arc<LineIndex>) -> lsp::TextEdit {
    lsp::TextEdit {
      range: self.span.to_range(line_index),
      new_text: self.new_text.clone(),
    }
  }

  pub fn as_text_or_annotated_text_edit(
    &self,
    line_index: Arc<LineIndex>,
  ) -> lsp::OneOf<lsp::TextEdit, lsp::AnnotatedTextEdit> {
    lsp::OneOf::Left(lsp::TextEdit {
      range: self.span.to_range(line_index),
      new_text: self.new_text.clone(),
    })
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileTextChanges {
  pub file_name: String,
  pub text_changes: Vec<TextChange>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub is_new_file: Option<bool>,
}

impl FileTextChanges {
  pub fn to_text_document_edit(
    &self,
    language_server: &language_server::Inner,
  ) -> Result<lsp::TextDocumentEdit, AnyError> {
    let specifier = normalize_specifier(&self.file_name)?;
    let asset_or_doc = language_server.get_asset_or_document(&specifier)?;
    let edits = self
      .text_changes
      .iter()
      .map(|tc| tc.as_text_or_annotated_text_edit(asset_or_doc.line_index()))
      .collect();
    Ok(lsp::TextDocumentEdit {
      text_document: lsp::OptionalVersionedTextDocumentIdentifier {
        uri: specifier,
        version: asset_or_doc.document_lsp_version(),
      },
      edits,
    })
  }

  pub fn to_text_document_change_ops(
    &self,
    language_server: &language_server::Inner,
  ) -> Result<Vec<lsp::DocumentChangeOperation>, AnyError> {
    let mut ops = Vec::<lsp::DocumentChangeOperation>::new();
    let specifier = normalize_specifier(&self.file_name)?;
    let maybe_asset_or_document = if !self.is_new_file.unwrap_or(false) {
      let asset_or_doc = language_server.get_asset_or_document(&specifier)?;
      Some(asset_or_doc)
    } else {
      None
    };
    let line_index = maybe_asset_or_document
      .as_ref()
      .map(|d| d.line_index())
      .unwrap_or_else(|| Arc::new(LineIndex::new("")));

    if self.is_new_file.unwrap_or(false) {
      ops.push(lsp::DocumentChangeOperation::Op(lsp::ResourceOp::Create(
        lsp::CreateFile {
          uri: specifier.clone(),
          options: Some(lsp::CreateFileOptions {
            ignore_if_exists: Some(true),
            overwrite: None,
          }),
          annotation_id: None,
        },
      )));
    }

    let edits = self
      .text_changes
      .iter()
      .map(|tc| tc.as_text_or_annotated_text_edit(line_index.clone()))
      .collect();
    ops.push(lsp::DocumentChangeOperation::Edit(lsp::TextDocumentEdit {
      text_document: lsp::OptionalVersionedTextDocumentIdentifier {
        uri: specifier,
        version: maybe_asset_or_document.and_then(|d| d.document_lsp_version()),
      },
      edits,
    }));

    Ok(ops)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Classifications {
  spans: Vec<u32>,
}

impl Classifications {
  pub fn to_semantic_tokens(
    &self,
    asset_or_doc: &AssetOrDocument,
    line_index: Arc<LineIndex>,
  ) -> LspResult<lsp::SemanticTokens> {
    let token_count = self.spans.len() / 3;
    let mut builder = SemanticTokensBuilder::new();
    for i in 0..token_count {
      let src_offset = 3 * i;
      let offset = self.spans[src_offset];
      let length = self.spans[src_offset + 1];
      let ts_classification = self.spans[src_offset + 2];

      let token_type =
        Classifications::get_token_type_from_classification(ts_classification);
      let token_modifiers =
        Classifications::get_token_modifier_from_classification(
          ts_classification,
        );

      let start_pos = line_index.position_tsc(offset.into());
      let end_pos = line_index.position_tsc(TextSize::from(offset + length));

      if start_pos.line == end_pos.line
        && start_pos.character <= end_pos.character
      {
        builder.push(
          start_pos.line,
          start_pos.character,
          end_pos.character - start_pos.character,
          token_type,
          token_modifiers,
        );
      } else {
        log::error!(
          "unexpected positions\nspecifier: {}\nopen: {}\nstart_pos: {:?}\nend_pos: {:?}",
          asset_or_doc.specifier(),
          asset_or_doc.is_open(),
          start_pos,
          end_pos
        );
        return Err(LspError::internal_error());
      }
    }
    Ok(builder.build(None))
  }

  fn get_token_type_from_classification(ts_classification: u32) -> u32 {
    assert!(ts_classification > semantic_tokens::MODIFIER_MASK);
    (ts_classification >> semantic_tokens::TYPE_OFFSET) - 1
  }

  fn get_token_modifier_from_classification(ts_classification: u32) -> u32 {
    ts_classification & semantic_tokens::MODIFIER_MASK
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefactorActionInfo {
  name: String,
  description: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  not_applicable_reason: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  kind: Option<String>,
}

impl RefactorActionInfo {
  pub fn get_action_kind(&self) -> lsp::CodeActionKind {
    if let Some(kind) = &self.kind {
      kind.clone().into()
    } else {
      let maybe_match = ALL_KNOWN_REFACTOR_ACTION_KINDS
        .iter()
        .find(|action| action.matches(&self.name));
      maybe_match
        .map_or(lsp::CodeActionKind::REFACTOR, |action| action.kind.clone())
    }
  }

  pub fn is_preferred(&self, all_actions: &[RefactorActionInfo]) -> bool {
    if EXTRACT_CONSTANT.matches(&self.name) {
      let get_scope = |name: &str| -> Option<u32> {
        if let Some(captures) = SCOPE_RE.captures(name) {
          captures[1].parse::<u32>().ok()
        } else {
          None
        }
      };

      return if let Some(scope) = get_scope(&self.name) {
        all_actions
          .iter()
          .filter(|other| {
            !std::ptr::eq(&self, other) && EXTRACT_CONSTANT.matches(&other.name)
          })
          .all(|other| {
            if let Some(other_scope) = get_scope(&other.name) {
              scope < other_scope
            } else {
              true
            }
          })
      } else {
        false
      };
    }
    if EXTRACT_TYPE.matches(&self.name) || EXTRACT_INTERFACE.matches(&self.name)
    {
      return true;
    }
    false
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicableRefactorInfo {
  name: String,
  // description: String,
  // #[serde(skip_serializing_if = "Option::is_none")]
  // inlineable: Option<bool>,
  actions: Vec<RefactorActionInfo>,
}

impl ApplicableRefactorInfo {
  pub fn to_code_actions(
    &self,
    specifier: &ModuleSpecifier,
    range: &lsp::Range,
  ) -> Vec<lsp::CodeAction> {
    let mut code_actions = Vec::<lsp::CodeAction>::new();
    // All typescript refactoring actions are inlineable
    for action in self.actions.iter() {
      code_actions
        .push(self.as_inline_code_action(action, specifier, range, &self.name));
    }
    code_actions
  }

  fn as_inline_code_action(
    &self,
    action: &RefactorActionInfo,
    specifier: &ModuleSpecifier,
    range: &lsp::Range,
    refactor_name: &str,
  ) -> lsp::CodeAction {
    let disabled = action.not_applicable_reason.as_ref().map(|reason| {
      lsp::CodeActionDisabled {
        reason: reason.clone(),
      }
    });

    lsp::CodeAction {
      title: action.description.to_string(),
      kind: Some(action.get_action_kind()),
      is_preferred: Some(action.is_preferred(&self.actions)),
      disabled,
      data: Some(
        serde_json::to_value(RefactorCodeActionData {
          specifier: specifier.clone(),
          range: *range,
          refactor_name: refactor_name.to_owned(),
          action_name: action.name.clone(),
        })
        .unwrap(),
      ),
      ..Default::default()
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefactorEditInfo {
  edits: Vec<FileTextChanges>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rename_location: Option<u32>,
}

impl RefactorEditInfo {
  pub async fn to_workspace_edit(
    &self,
    language_server: &language_server::Inner,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    let mut all_ops = Vec::<lsp::DocumentChangeOperation>::new();
    for edit in self.edits.iter() {
      let ops = edit.to_text_document_change_ops(language_server)?;
      all_ops.extend(ops);
    }

    Ok(Some(lsp::WorkspaceEdit {
      document_changes: Some(lsp::DocumentChanges::Operations(all_ops)),
      ..Default::default()
    }))
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAction {
  description: String,
  changes: Vec<FileTextChanges>,
  #[serde(skip_serializing_if = "Option::is_none")]
  commands: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeFixAction {
  pub description: String,
  pub changes: Vec<FileTextChanges>,
  // These are opaque types that should just be passed back when applying the
  // action.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub commands: Option<Vec<Value>>,
  pub fix_name: String,
  // It appears currently that all fixIds are strings, but the protocol
  // specifies an opaque type, the problem is that we need to use the id as a
  // hash key, and `Value` does not implement hash (and it could provide a false
  // positive depending on JSON whitespace, so we deserialize it but it might
  // break in the future)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fix_id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fix_all_description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinedCodeActions {
  pub changes: Vec<FileTextChanges>,
  pub commands: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceEntry {
  // is_write_access: bool,
  #[serde(default)]
  pub is_definition: bool,
  // is_in_string: Option<bool>,
  #[serde(flatten)]
  pub document_span: DocumentSpan,
}

impl ReferenceEntry {
  pub fn to_location(
    &self,
    line_index: Arc<LineIndex>,
    url_map: &LspUrlMap,
  ) -> lsp::Location {
    let specifier = normalize_specifier(&self.document_span.file_name)
      .unwrap_or_else(|_| INVALID_SPECIFIER.clone());
    let uri = url_map
      .normalize_specifier(&specifier)
      .unwrap_or_else(|_| INVALID_SPECIFIER.clone());
    lsp::Location {
      uri,
      range: self.document_span.text_span.to_range(line_index),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyItem {
  name: String,
  kind: ScriptElementKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  kind_modifiers: Option<String>,
  file: String,
  span: TextSpan,
  selection_span: TextSpan,
  #[serde(skip_serializing_if = "Option::is_none")]
  container_name: Option<String>,
}

impl CallHierarchyItem {
  pub fn try_resolve_call_hierarchy_item(
    &self,
    language_server: &language_server::Inner,
    maybe_root_path: Option<&Path>,
  ) -> Option<lsp::CallHierarchyItem> {
    let target_specifier = normalize_specifier(&self.file).ok()?;
    let target_asset_or_doc =
      language_server.get_maybe_asset_or_document(&target_specifier)?;

    Some(self.to_call_hierarchy_item(
      target_asset_or_doc.line_index(),
      language_server,
      maybe_root_path,
    ))
  }

  pub fn to_call_hierarchy_item(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
    maybe_root_path: Option<&Path>,
  ) -> lsp::CallHierarchyItem {
    let target_specifier = normalize_specifier(&self.file)
      .unwrap_or_else(|_| INVALID_SPECIFIER.clone());
    let uri = language_server
      .url_map
      .normalize_specifier(&target_specifier)
      .unwrap_or_else(|_| INVALID_SPECIFIER.clone());

    let use_file_name = self.is_source_file_item();
    let maybe_file_path = if uri.scheme() == "file" {
      specifier_to_file_path(&uri).ok()
    } else {
      None
    };
    let name = if use_file_name {
      if let Some(file_path) = maybe_file_path.as_ref() {
        file_path.file_name().unwrap().to_string_lossy().to_string()
      } else {
        uri.to_string()
      }
    } else {
      self.name.clone()
    };
    let detail = if use_file_name {
      if let Some(file_path) = maybe_file_path.as_ref() {
        // TODO: update this to work with multi root workspaces
        let parent_dir = file_path.parent().unwrap();
        if let Some(root_path) = maybe_root_path {
          parent_dir
            .strip_prefix(root_path)
            .unwrap_or(parent_dir)
            .to_string_lossy()
            .to_string()
        } else {
          parent_dir.to_string_lossy().to_string()
        }
      } else {
        String::new()
      }
    } else {
      self.container_name.as_ref().cloned().unwrap_or_default()
    };

    let mut tags: Option<Vec<lsp::SymbolTag>> = None;
    if let Some(modifiers) = self.kind_modifiers.as_ref() {
      let kind_modifiers = parse_kind_modifier(modifiers);
      if kind_modifiers.contains("deprecated") {
        tags = Some(vec![lsp::SymbolTag::DEPRECATED]);
      }
    }

    lsp::CallHierarchyItem {
      name,
      tags,
      uri,
      detail: Some(detail),
      kind: self.kind.clone().into(),
      range: self.span.to_range(line_index.clone()),
      selection_range: self.selection_span.to_range(line_index),
      data: None,
    }
  }

  fn is_source_file_item(&self) -> bool {
    self.kind == ScriptElementKind::ScriptElement
      || self.kind == ScriptElementKind::ModuleElement
        && self.selection_span.start == 0
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyIncomingCall {
  from: CallHierarchyItem,
  from_spans: Vec<TextSpan>,
}

impl CallHierarchyIncomingCall {
  pub fn try_resolve_call_hierarchy_incoming_call(
    &self,
    language_server: &language_server::Inner,
    maybe_root_path: Option<&Path>,
  ) -> Option<lsp::CallHierarchyIncomingCall> {
    let target_specifier = normalize_specifier(&self.from.file).ok()?;
    let target_asset_or_doc =
      language_server.get_maybe_asset_or_document(&target_specifier)?;

    Some(lsp::CallHierarchyIncomingCall {
      from: self.from.to_call_hierarchy_item(
        target_asset_or_doc.line_index(),
        language_server,
        maybe_root_path,
      ),
      from_ranges: self
        .from_spans
        .iter()
        .map(|span| span.to_range(target_asset_or_doc.line_index()))
        .collect(),
    })
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyOutgoingCall {
  to: CallHierarchyItem,
  from_spans: Vec<TextSpan>,
}

impl CallHierarchyOutgoingCall {
  pub fn try_resolve_call_hierarchy_outgoing_call(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
    maybe_root_path: Option<&Path>,
  ) -> Option<lsp::CallHierarchyOutgoingCall> {
    let target_specifier = normalize_specifier(&self.to.file).ok()?;
    let target_asset_or_doc =
      language_server.get_maybe_asset_or_document(&target_specifier)?;

    Some(lsp::CallHierarchyOutgoingCall {
      to: self.to.to_call_hierarchy_item(
        target_asset_or_doc.line_index(),
        language_server,
        maybe_root_path,
      ),
      from_ranges: self
        .from_spans
        .iter()
        .map(|span| span.to_range(line_index.clone()))
        .collect(),
    })
  }
}

/// Used to convert completion code actions into a command and additional text
/// edits to pass in the completion item.
fn parse_code_actions(
  maybe_code_actions: Option<&Vec<CodeAction>>,
  data: &CompletionItemData,
  specifier: &ModuleSpecifier,
  language_server: &language_server::Inner,
) -> Result<(Option<lsp::Command>, Option<Vec<lsp::TextEdit>>), AnyError> {
  if let Some(code_actions) = maybe_code_actions {
    let mut additional_text_edits: Vec<lsp::TextEdit> = Vec::new();
    let mut has_remaining_commands_or_edits = false;
    for ts_action in code_actions {
      if ts_action.commands.is_some() {
        has_remaining_commands_or_edits = true;
      }

      let asset_or_doc =
        language_server.get_asset_or_document(&data.specifier)?;
      for change in &ts_action.changes {
        let change_specifier = normalize_specifier(&change.file_name)?;
        if data.specifier == change_specifier {
          additional_text_edits.extend(change.text_changes.iter().map(|tc| {
            update_import_statement(
              tc.as_text_edit(asset_or_doc.line_index()),
              data,
            )
          }));
        } else {
          has_remaining_commands_or_edits = true;
        }
      }
    }

    let mut command: Option<lsp::Command> = None;
    if has_remaining_commands_or_edits {
      let actions: Vec<Value> = code_actions
        .iter()
        .map(|ca| {
          let changes: Vec<FileTextChanges> = ca
            .changes
            .clone()
            .into_iter()
            .filter(|ch| {
              normalize_specifier(&ch.file_name).unwrap() == data.specifier
            })
            .collect();
          json!({
            "commands": ca.commands,
            "description": ca.description,
            "changes": changes,
          })
        })
        .collect();
      command = Some(lsp::Command {
        title: "".to_string(),
        command: "_typescript.applyCompletionCodeAction".to_string(),
        arguments: Some(vec![json!(specifier.to_string()), json!(actions)]),
      });
    }

    if additional_text_edits.is_empty() {
      Ok((command, None))
    } else {
      Ok((command, Some(additional_text_edits)))
    }
  } else {
    Ok((None, None))
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionEntryDetails {
  display_parts: Vec<SymbolDisplayPart>,
  documentation: Option<Vec<SymbolDisplayPart>>,
  tags: Option<Vec<JsDocTagInfo>>,
  name: String,
  kind: ScriptElementKind,
  kind_modifiers: String,
  code_actions: Option<Vec<CodeAction>>,
  source_display: Option<Vec<SymbolDisplayPart>>,
}

impl CompletionEntryDetails {
  pub fn as_completion_item(
    &self,
    original_item: &lsp::CompletionItem,
    data: &CompletionItemData,
    specifier: &ModuleSpecifier,
    language_server: &language_server::Inner,
  ) -> Result<lsp::CompletionItem, AnyError> {
    let detail = if original_item.detail.is_some() {
      original_item.detail.clone()
    } else if !self.display_parts.is_empty() {
      Some(replace_links(&display_parts_to_string(
        &self.display_parts,
        language_server,
      )))
    } else {
      None
    };
    let documentation = if let Some(parts) = &self.documentation {
      let mut value = display_parts_to_string(parts, language_server);
      if let Some(tags) = &self.tags {
        let tag_documentation = tags
          .iter()
          .map(|tag_info| get_tag_documentation(tag_info, language_server))
          .collect::<Vec<String>>()
          .join("");
        value = format!("{}\n\n{}", value, tag_documentation);
      }
      Some(lsp::Documentation::MarkupContent(lsp::MarkupContent {
        kind: lsp::MarkupKind::Markdown,
        value,
      }))
    } else {
      None
    };
    let (command, additional_text_edits) = parse_code_actions(
      self.code_actions.as_ref(),
      data,
      specifier,
      language_server,
    )?;
    // TODO(@kitsonk) add `use_code_snippet`

    Ok(lsp::CompletionItem {
      data: None,
      detail,
      documentation,
      command,
      additional_text_edits,
      ..original_item.clone()
    })
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionInfo {
  entries: Vec<CompletionEntry>,
  // this is only used by Microsoft's telemetrics, which Deno doesn't use and
  // there are issues with the value not matching the type definitions.
  // flags: Option<CompletionInfoFlags>,
  is_global_completion: bool,
  is_member_completion: bool,
  is_new_identifier_location: bool,
  metadata: Option<Value>,
  optional_replacement_span: Option<TextSpan>,
}

impl CompletionInfo {
  pub fn as_completion_response(
    &self,
    line_index: Arc<LineIndex>,
    settings: &config::CompletionSettings,
    specifier: &ModuleSpecifier,
    position: u32,
  ) -> lsp::CompletionResponse {
    let items = self
      .entries
      .iter()
      .map(|entry| {
        entry.as_completion_item(
          line_index.clone(),
          self,
          settings,
          specifier,
          position,
        )
      })
      .collect();
    let is_incomplete = self
      .metadata
      .clone()
      .map(|v| {
        v.as_object()
          .unwrap()
          .get("isIncomplete")
          .unwrap_or(&json!(false))
          .as_bool()
          .unwrap()
      })
      .unwrap_or(false);
    lsp::CompletionResponse::List(lsp::CompletionList {
      is_incomplete,
      items,
    })
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemData {
  pub specifier: ModuleSpecifier,
  pub position: u32,
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<Value>,
  pub use_code_snippet: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletionEntryDataImport {
  module_specifier: String,
  file_name: String,
}

/// Modify an import statement text replacement to have the correct import
/// specifier to work with Deno module resolution.
fn update_import_statement(
  mut text_edit: lsp::TextEdit,
  item_data: &CompletionItemData,
) -> lsp::TextEdit {
  if let Some(data) = &item_data.data {
    if let Ok(import_data) =
      serde_json::from_value::<CompletionEntryDataImport>(data.clone())
    {
      if let Ok(import_specifier) = normalize_specifier(&import_data.file_name)
      {
        let new_module_specifier =
          relative_specifier(&import_specifier, &item_data.specifier);
        text_edit.new_text = text_edit
          .new_text
          .replace(&import_data.module_specifier, &new_module_specifier);
      }
    }
  }
  text_edit
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionEntry {
  name: String,
  kind: ScriptElementKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  kind_modifiers: Option<String>,
  sort_text: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  insert_text: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_snippet: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  replacement_span: Option<TextSpan>,
  #[serde(skip_serializing_if = "Option::is_none")]
  has_action: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  source: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  source_display: Option<Vec<SymbolDisplayPart>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  label_details: Option<CompletionEntryLabelDetails>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_recommended: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_from_unchecked_file: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_package_json_import: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_import_statement_completion: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  data: Option<Value>,
}

impl CompletionEntry {
  fn get_commit_characters(
    &self,
    info: &CompletionInfo,
    settings: &config::CompletionSettings,
  ) -> Option<Vec<String>> {
    if info.is_new_identifier_location {
      return None;
    }

    let mut commit_characters = vec![];
    match self.kind {
      ScriptElementKind::MemberGetAccessorElement
      | ScriptElementKind::MemberSetAccessorElement
      | ScriptElementKind::ConstructSignatureElement
      | ScriptElementKind::CallSignatureElement
      | ScriptElementKind::IndexSignatureElement
      | ScriptElementKind::EnumElement
      | ScriptElementKind::InterfaceElement => {
        commit_characters.push(".");
        commit_characters.push(";");
      }
      ScriptElementKind::ModuleElement
      | ScriptElementKind::Alias
      | ScriptElementKind::ConstElement
      | ScriptElementKind::LetElement
      | ScriptElementKind::VariableElement
      | ScriptElementKind::LocalVariableElement
      | ScriptElementKind::MemberVariableElement
      | ScriptElementKind::ClassElement
      | ScriptElementKind::FunctionElement
      | ScriptElementKind::MemberFunctionElement
      | ScriptElementKind::Keyword
      | ScriptElementKind::ParameterElement => {
        commit_characters.push(".");
        commit_characters.push(",");
        commit_characters.push(";");
        if !settings.complete_function_calls {
          commit_characters.push("(");
        }
      }
      _ => (),
    }

    if commit_characters.is_empty() {
      None
    } else {
      Some(commit_characters.into_iter().map(String::from).collect())
    }
  }

  fn get_filter_text(&self) -> Option<String> {
    if self.name.starts_with('#') {
      if let Some(insert_text) = &self.insert_text {
        if insert_text.starts_with("this.#") {
          return Some(insert_text.replace("this.#", ""));
        } else {
          return Some(insert_text.clone());
        }
      } else {
        return Some(self.name.replace('#', ""));
      }
    }

    if let Some(insert_text) = &self.insert_text {
      if insert_text.starts_with("this.") {
        return None;
      }
      if insert_text.starts_with('[') {
        return Some(
          BRACKET_ACCESSOR_RE
            .replace(insert_text, |caps: &Captures| format!(".{}", &caps[1]))
            .to_string(),
        );
      }
    }

    self.insert_text.clone()
  }

  pub fn as_completion_item(
    &self,
    line_index: Arc<LineIndex>,
    info: &CompletionInfo,
    settings: &config::CompletionSettings,
    specifier: &ModuleSpecifier,
    position: u32,
  ) -> lsp::CompletionItem {
    let mut label = self.name.clone();
    let mut kind: Option<lsp::CompletionItemKind> =
      Some(self.kind.clone().into());

    let sort_text = if self.source.is_some() {
      Some(format!("\u{ffff}{}", self.sort_text))
    } else {
      Some(self.sort_text.clone())
    };

    let preselect = self.is_recommended;
    let use_code_snippet = settings.complete_function_calls
      && (kind == Some(lsp::CompletionItemKind::FUNCTION)
        || kind == Some(lsp::CompletionItemKind::METHOD));
    let commit_characters = self.get_commit_characters(info, settings);
    let mut insert_text = self.insert_text.clone();
    let insert_text_format = match self.is_snippet {
      Some(true) => Some(lsp::InsertTextFormat::SNIPPET),
      _ => None,
    };
    let range = self.replacement_span.clone();
    let mut filter_text = self.get_filter_text();
    let mut tags = None;
    let mut detail = None;

    if let Some(kind_modifiers) = &self.kind_modifiers {
      let kind_modifiers = parse_kind_modifier(kind_modifiers);
      if kind_modifiers.contains("optional") {
        if insert_text.is_none() {
          insert_text = Some(label.clone());
        }
        if filter_text.is_none() {
          filter_text = Some(label.clone());
        }
        label += "?";
      }
      if kind_modifiers.contains("deprecated") {
        tags = Some(vec![lsp::CompletionItemTag::DEPRECATED]);
      }
      if kind_modifiers.contains("color") {
        kind = Some(lsp::CompletionItemKind::COLOR);
      }
      if self.kind == ScriptElementKind::ScriptElement {
        for ext_modifier in FILE_EXTENSION_KIND_MODIFIERS {
          if kind_modifiers.contains(ext_modifier) {
            detail = if self.name.to_lowercase().ends_with(ext_modifier) {
              Some(self.name.clone())
            } else {
              Some(format!("{}{}", self.name, ext_modifier))
            };
            break;
          }
        }
      }
    }

    let text_edit =
      if let (Some(text_span), Some(new_text)) = (range, &insert_text) {
        let range = text_span.to_range(line_index);
        let insert_replace_edit = lsp::InsertReplaceEdit {
          new_text: new_text.clone(),
          insert: range,
          replace: range,
        };
        Some(insert_replace_edit.into())
      } else {
        None
      };

    let tsc = CompletionItemData {
      specifier: specifier.clone(),
      position,
      name: self.name.clone(),
      source: self.source.clone(),
      data: self.data.clone(),
      use_code_snippet,
    };

    lsp::CompletionItem {
      label,
      kind,
      sort_text,
      preselect,
      text_edit,
      filter_text,
      insert_text,
      insert_text_format,
      detail,
      tags,
      commit_characters,
      data: Some(json!({ "tsc": tsc })),
      ..Default::default()
    }
  }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletionEntryLabelDetails {
  #[serde(skip_serializing_if = "Option::is_none")]
  detail: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub enum OutliningSpanKind {
  #[serde(rename = "comment")]
  Comment,
  #[serde(rename = "region")]
  Region,
  #[serde(rename = "code")]
  Code,
  #[serde(rename = "imports")]
  Imports,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutliningSpan {
  text_span: TextSpan,
  // hint_span: TextSpan,
  // banner_text: String,
  // auto_collapse: bool,
  kind: OutliningSpanKind,
}

const FOLD_END_PAIR_CHARACTERS: &[u8] = &[b'}', b']', b')', b'`'];

impl OutliningSpan {
  pub fn to_folding_range(
    &self,
    line_index: Arc<LineIndex>,
    content: &[u8],
    line_folding_only: bool,
  ) -> lsp::FoldingRange {
    let range = self.text_span.to_range(line_index.clone());
    lsp::FoldingRange {
      start_line: range.start.line,
      start_character: if line_folding_only {
        None
      } else {
        Some(range.start.character)
      },
      end_line: self.adjust_folding_end_line(
        &range,
        line_index,
        content,
        line_folding_only,
      ),
      end_character: if line_folding_only {
        None
      } else {
        Some(range.end.character)
      },
      kind: self.get_folding_range_kind(&self.kind),
    }
  }

  fn adjust_folding_end_line(
    &self,
    range: &lsp::Range,
    line_index: Arc<LineIndex>,
    content: &[u8],
    line_folding_only: bool,
  ) -> u32 {
    if line_folding_only && range.end.line > 0 && range.end.character > 0 {
      let offset_end: usize = line_index.offset(range.end).unwrap().into();
      let fold_end_char = content[offset_end - 1];
      if FOLD_END_PAIR_CHARACTERS.contains(&fold_end_char) {
        return cmp::max(range.end.line - 1, range.start.line);
      }
    }

    range.end.line
  }

  fn get_folding_range_kind(
    &self,
    span_kind: &OutliningSpanKind,
  ) -> Option<lsp::FoldingRangeKind> {
    match span_kind {
      OutliningSpanKind::Comment => Some(lsp::FoldingRangeKind::Comment),
      OutliningSpanKind::Region => Some(lsp::FoldingRangeKind::Region),
      OutliningSpanKind::Imports => Some(lsp::FoldingRangeKind::Imports),
      _ => None,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpItems {
  items: Vec<SignatureHelpItem>,
  // applicable_span: TextSpan,
  selected_item_index: u32,
  argument_index: u32,
  // argument_count: u32,
}

impl SignatureHelpItems {
  pub fn into_signature_help(
    self,
    language_server: &language_server::Inner,
  ) -> lsp::SignatureHelp {
    lsp::SignatureHelp {
      signatures: self
        .items
        .into_iter()
        .map(|item| item.into_signature_information(language_server))
        .collect(),
      active_parameter: Some(self.argument_index),
      active_signature: Some(self.selected_item_index),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpItem {
  // is_variadic: bool,
  prefix_display_parts: Vec<SymbolDisplayPart>,
  suffix_display_parts: Vec<SymbolDisplayPart>,
  // separator_display_parts: Vec<SymbolDisplayPart>,
  parameters: Vec<SignatureHelpParameter>,
  documentation: Vec<SymbolDisplayPart>,
  // tags: Vec<JsDocTagInfo>,
}

impl SignatureHelpItem {
  pub fn into_signature_information(
    self,
    language_server: &language_server::Inner,
  ) -> lsp::SignatureInformation {
    let prefix_text =
      display_parts_to_string(&self.prefix_display_parts, language_server);
    let params_text = self
      .parameters
      .iter()
      .map(|param| {
        display_parts_to_string(&param.display_parts, language_server)
      })
      .collect::<Vec<String>>()
      .join(", ");
    let suffix_text =
      display_parts_to_string(&self.suffix_display_parts, language_server);
    let documentation =
      display_parts_to_string(&self.documentation, language_server);
    lsp::SignatureInformation {
      label: format!("{}{}{}", prefix_text, params_text, suffix_text),
      documentation: Some(lsp::Documentation::MarkupContent(
        lsp::MarkupContent {
          kind: lsp::MarkupKind::Markdown,
          value: documentation,
        },
      )),
      parameters: Some(
        self
          .parameters
          .into_iter()
          .map(|param| param.into_parameter_information(language_server))
          .collect(),
      ),
      active_parameter: None,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpParameter {
  // name: String,
  documentation: Vec<SymbolDisplayPart>,
  display_parts: Vec<SymbolDisplayPart>,
  // is_optional: bool,
}

impl SignatureHelpParameter {
  pub fn into_parameter_information(
    self,
    language_server: &language_server::Inner,
  ) -> lsp::ParameterInformation {
    let documentation =
      display_parts_to_string(&self.documentation, language_server);
    lsp::ParameterInformation {
      label: lsp::ParameterLabel::Simple(display_parts_to_string(
        &self.display_parts,
        language_server,
      )),
      documentation: Some(lsp::Documentation::MarkupContent(
        lsp::MarkupContent {
          kind: lsp::MarkupKind::Markdown,
          value: documentation,
        },
      )),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRange {
  text_span: TextSpan,
  #[serde(skip_serializing_if = "Option::is_none")]
  parent: Option<Box<SelectionRange>>,
}

impl SelectionRange {
  pub fn to_selection_range(
    &self,
    line_index: Arc<LineIndex>,
  ) -> lsp::SelectionRange {
    lsp::SelectionRange {
      range: self.text_span.to_range(line_index.clone()),
      parent: self.parent.as_ref().map(|parent_selection| {
        Box::new(parent_selection.to_selection_range(line_index))
      }),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
  // id: usize,
  data: Value,
}

struct State {
  last_id: usize,
  performance: Arc<Performance>,
  response: Option<Response>,
  state_snapshot: Arc<StateSnapshot>,
  specifiers: HashMap<String, String>,
  token: CancellationToken,
}

impl State {
  fn new(
    state_snapshot: Arc<StateSnapshot>,
    performance: Arc<Performance>,
  ) -> Self {
    Self {
      last_id: 1,
      performance,
      response: None,
      state_snapshot,
      specifiers: HashMap::default(),
      token: Default::default(),
    }
  }

  /// If a normalized version of the specifier has been stored for tsc, this
  /// will "restore" it for communicating back to the tsc language server,
  /// otherwise it will just convert the specifier to a string.
  fn denormalize_specifier(&self, specifier: &ModuleSpecifier) -> String {
    let specifier_str = specifier.to_string();
    self
      .specifiers
      .get(&specifier_str)
      .unwrap_or(&specifier_str)
      .to_string()
  }

  /// In certain situations, tsc can request "invalid" specifiers and this will
  /// normalize and memoize the specifier.
  fn normalize_specifier<S: AsRef<str>>(
    &mut self,
    specifier: S,
  ) -> Result<ModuleSpecifier, AnyError> {
    let specifier_str = specifier.as_ref().replace(".d.ts.d.ts", ".d.ts");
    if specifier_str != specifier.as_ref() {
      self
        .specifiers
        .insert(specifier_str.clone(), specifier.as_ref().to_string());
    }
    ModuleSpecifier::parse(&specifier_str).map_err(|err| err.into())
  }

  fn get_asset_or_document(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<AssetOrDocument> {
    let snapshot = &self.state_snapshot;
    if specifier.scheme() == "asset" {
      snapshot.assets.get(specifier).map(AssetOrDocument::Asset)
    } else {
      snapshot
        .documents
        .get(specifier)
        .map(AssetOrDocument::Document)
    }
  }

  fn script_version(&self, specifier: &ModuleSpecifier) -> Option<String> {
    if specifier.scheme() == "asset" {
      if self.state_snapshot.assets.contains_key(specifier) {
        Some("1".to_string())
      } else {
        None
      }
    } else {
      self
        .state_snapshot
        .documents
        .get(specifier)
        .map(|d| d.script_version())
    }
  }
}

fn normalize_specifier<S: AsRef<str>>(
  specifier: S,
) -> Result<ModuleSpecifier, AnyError> {
  resolve_url(specifier.as_ref().replace(".d.ts.d.ts", ".d.ts").as_str())
    .map_err(|err| err.into())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpecifierArgs {
  specifier: String,
}

#[op]
fn op_exists(state: &mut OpState, args: SpecifierArgs) -> bool {
  let state = state.borrow_mut::<State>();
  // we don't measure the performance of op_exists anymore because as of TS 4.5
  // it is noisy with all the checking for custom libs, that we can't see the
  // forrest for the trees as well as it compounds any lsp performance
  // challenges, opening a single document in the editor causes some 3k worth
  // of op_exists requests... :omg:
  let specifier = match state.normalize_specifier(&args.specifier) {
    Ok(url) => url,
    // sometimes tsc tries to query invalid specifiers, especially when
    // something else isn't quite right, so instead of bubbling up the error
    // back to tsc, we simply swallow it and say the file doesn't exist
    Err(_) => return false,
  };
  state.state_snapshot.documents.exists(&specifier)
}

#[op]
fn op_is_cancelled(state: &mut OpState) -> bool {
  let state = state.borrow_mut::<State>();
  state.token.is_cancelled()
}

#[op]
fn op_load(
  state: &mut OpState,
  args: SpecifierArgs,
) -> Result<Value, AnyError> {
  let state = state.borrow_mut::<State>();
  let mark = state.performance.mark("op_load", Some(&args));
  let specifier = state.normalize_specifier(args.specifier)?;
  let asset_or_document = state.get_asset_or_document(&specifier);
  state.performance.measure(mark);
  Ok(match asset_or_document {
    Some(doc) => {
      json!({
        "data": doc.text(),
        "scriptKind": crate::tsc::as_ts_script_kind(&doc.media_type()),
        "version": state.script_version(&specifier),
      })
    }
    None => Value::Null,
  })
}

#[op]
fn op_resolve(
  state: &mut OpState,
  args: ResolveArgs,
) -> Result<Vec<Option<(String, String)>>, AnyError> {
  let state = state.borrow_mut::<State>();
  let mark = state.performance.mark("op_resolve", Some(&args));
  let referrer = state.normalize_specifier(&args.base)?;

  let result = if let Some(resolved) = state
    .state_snapshot
    .documents
    .resolve(args.specifiers, &referrer)
  {
    Ok(
      resolved
        .into_iter()
        .map(|o| {
          o.map(|(s, mt)| (s.to_string(), mt.as_ts_extension().to_string()))
        })
        .collect(),
    )
  } else {
    Err(custom_error(
      "NotFound",
      format!(
        "Error resolving. Referring specifier \"{}\" was not found.",
        args.base
      ),
    ))
  };

  state.performance.measure(mark);
  result
}

#[op]
fn op_respond(state: &mut OpState, args: Response) -> bool {
  let state = state.borrow_mut::<State>();
  state.response = Some(args);
  true
}

#[op]
fn op_script_names(state: &mut OpState) -> Vec<ModuleSpecifier> {
  let state = state.borrow_mut::<State>();
  state
    .state_snapshot
    .documents
    .documents(true, true)
    .into_iter()
    .map(|d| d.specifier().clone())
    .collect()
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScriptVersionArgs {
  specifier: String,
}

#[op]
fn op_script_version(
  state: &mut OpState,
  args: ScriptVersionArgs,
) -> Result<Option<String>, AnyError> {
  let state = state.borrow_mut::<State>();
  // this op is very "noisy" and measuring its performance is not useful, so we
  // don't measure it uniquely anymore.
  let specifier = state.normalize_specifier(args.specifier)?;
  Ok(state.script_version(&specifier))
}

/// Create and setup a JsRuntime based on a snapshot. It is expected that the
/// supplied snapshot is an isolate that contains the TypeScript language
/// server.
fn js_runtime(performance: Arc<Performance>) -> JsRuntime {
  JsRuntime::new(RuntimeOptions {
    extensions: vec![init_extension(performance)],
    startup_snapshot: Some(tsc::compiler_snapshot()),
    ..Default::default()
  })
}

fn init_extension(performance: Arc<Performance>) -> Extension {
  Extension::builder()
    .ops(vec![
      op_exists::decl(),
      op_is_cancelled::decl(),
      op_load::decl(),
      op_resolve::decl(),
      op_respond::decl(),
      op_script_names::decl(),
      op_script_version::decl(),
    ])
    .state(move |state| {
      state.put(State::new(
        Arc::new(StateSnapshot::default()),
        performance.clone(),
      ));
      Ok(())
    })
    .build()
}

/// Instruct a language server runtime to start the language server and provide
/// it with a minimal bootstrap configuration.
fn start(
  runtime: &mut JsRuntime,
  debug: bool,
  state_snapshot: &StateSnapshot,
) -> Result<(), AnyError> {
  let root_uri = state_snapshot
    .root_uri
    .clone()
    .unwrap_or_else(|| Url::parse("cache:///").unwrap());
  let init_config = json!({ "debug": debug, "rootUri": root_uri });
  let init_src = format!("globalThis.serverInit({});", init_config);

  runtime.execute_script(&located_script_name!(), &init_src)?;
  Ok(())
}

#[derive(Debug, Deserialize_repr, Serialize_repr)]
#[repr(u32)]
pub enum CompletionTriggerKind {
  Invoked = 1,
  TriggerCharacter = 2,
  TriggerForIncompleteCompletions = 3,
}

impl From<lsp::CompletionTriggerKind> for CompletionTriggerKind {
  fn from(kind: lsp::CompletionTriggerKind) -> Self {
    match kind {
      lsp::CompletionTriggerKind::INVOKED => Self::Invoked,
      lsp::CompletionTriggerKind::TRIGGER_CHARACTER => Self::TriggerCharacter,
      lsp::CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS => {
        Self::TriggerForIncompleteCompletions
      }
      _ => Self::Invoked,
    }
  }
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
pub enum IncludeInlayParameterNameHints {
  None,
  Literals,
  All,
}

impl From<&config::InlayHintsParamNamesEnabled>
  for IncludeInlayParameterNameHints
{
  fn from(setting: &config::InlayHintsParamNamesEnabled) -> Self {
    match setting {
      config::InlayHintsParamNamesEnabled::All => Self::All,
      config::InlayHintsParamNamesEnabled::Literals => Self::Literals,
      config::InlayHintsParamNamesEnabled::None => Self::None,
    }
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum IncludePackageJsonAutoImports {
  Auto,
  On,
  Off,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum JsxAttributeCompletionStyle {
  Auto,
  Braces,
  None,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompletionsAtPositionOptions {
  #[serde(flatten)]
  pub user_preferences: UserPreferences,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_character: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_kind: Option<CompletionTriggerKind>,
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
  pub include_completions_for_import_statements: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_snippet_text: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_automatic_optional_chain_completions: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_insert_text: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_class_member_snippets: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_object_literal_method_snippets: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub use_label_details_in_completion_entries: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub allow_incomplete_completions: Option<bool>,
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub jsx_attribute_completion_style: Option<JsxAttributeCompletionStyle>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_parameter_name_hints:
    Option<IncludeInlayParameterNameHints>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_parameter_name_hints_when_argument_matches_name:
    Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_function_parameter_type_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_variable_type_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_variable_type_hints_when_type_matches_name: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_property_declaration_type_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_function_like_return_type_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_enum_member_value_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub allow_rename_of_import_path: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub auto_import_file_exclude_patterns: Option<Vec<String>>,
}

impl From<&config::WorkspaceSettings> for UserPreferences {
  fn from(workspace_settings: &config::WorkspaceSettings) -> Self {
    let inlay_hints = &workspace_settings.inlay_hints;
    Self {
      include_inlay_parameter_name_hints: Some(
        (&inlay_hints.parameter_names.enabled).into(),
      ),
      include_inlay_parameter_name_hints_when_argument_matches_name: Some(
        inlay_hints
          .parameter_names
          .suppress_when_argument_matches_name,
      ),
      include_inlay_function_parameter_type_hints: Some(
        inlay_hints.parameter_types.enabled,
      ),
      include_inlay_variable_type_hints: Some(
        inlay_hints.variable_types.enabled,
      ),
      include_inlay_variable_type_hints_when_type_matches_name: Some(
        inlay_hints
          .variable_types
          .suppress_when_argument_matches_name,
      ),
      include_inlay_property_declaration_type_hints: Some(
        inlay_hints.property_declaration_types.enabled,
      ),
      include_inlay_function_like_return_type_hints: Some(
        inlay_hints.function_like_return_types.enabled,
      ),
      include_inlay_enum_member_value_hints: Some(
        inlay_hints.enum_member_values.enabled,
      ),
      ..Default::default()
    }
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpItemsOptions {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_reason: Option<SignatureHelpTriggerReason>,
}

#[derive(Debug, Serialize)]
pub enum SignatureHelpTriggerKind {
  #[serde(rename = "characterTyped")]
  CharacterTyped,
  #[serde(rename = "invoked")]
  Invoked,
  #[serde(rename = "retrigger")]
  Retrigger,
  #[serde(rename = "unknown")]
  Unknown,
}

impl From<lsp::SignatureHelpTriggerKind> for SignatureHelpTriggerKind {
  fn from(kind: lsp::SignatureHelpTriggerKind) -> Self {
    match kind {
      lsp::SignatureHelpTriggerKind::INVOKED => Self::Invoked,
      lsp::SignatureHelpTriggerKind::TRIGGER_CHARACTER => Self::CharacterTyped,
      lsp::SignatureHelpTriggerKind::CONTENT_CHANGE => Self::Retrigger,
      _ => Self::Unknown,
    }
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpTriggerReason {
  pub kind: SignatureHelpTriggerKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_character: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompletionDetailsArgs {
  pub specifier: ModuleSpecifier,
  pub position: u32,
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub preferences: Option<UserPreferences>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<Value>,
}

impl From<&CompletionItemData> for GetCompletionDetailsArgs {
  fn from(item_data: &CompletionItemData) -> Self {
    Self {
      specifier: item_data.specifier.clone(),
      position: item_data.position,
      name: item_data.name.clone(),
      source: item_data.source.clone(),
      preferences: None,
      data: item_data.data.clone(),
    }
  }
}

/// Methods that are supported by the Language Service in the compiler isolate.
#[derive(Debug)]
pub enum RequestMethod {
  /// Configure the compilation settings for the server.
  Configure(TsConfig),
  /// Get rename locations at a given position.
  FindRenameLocations {
    specifier: ModuleSpecifier,
    position: u32,
    find_in_strings: bool,
    find_in_comments: bool,
    provide_prefix_and_suffix_text_for_rename: bool,
  },
  GetAssets,
  /// Retrieve the possible refactor info for a range of a file.
  GetApplicableRefactors((ModuleSpecifier, TextSpan, String)),
  /// Retrieve the refactor edit info for a range.
  GetEditsForRefactor((ModuleSpecifier, TextSpan, String, String)),
  /// Retrieve code fixes for a range of a file with the provided error codes.
  GetCodeFixes((ModuleSpecifier, u32, u32, Vec<String>)),
  /// Get completion information at a given position (IntelliSense).
  GetCompletions((ModuleSpecifier, u32, GetCompletionsAtPositionOptions)),
  /// Get details about a specific completion entry.
  GetCompletionDetails(GetCompletionDetailsArgs),
  /// Retrieve the combined code fixes for a fix id for a module.
  GetCombinedCodeFix((ModuleSpecifier, Value)),
  /// Get declaration information for a specific position.
  GetDefinition((ModuleSpecifier, u32)),
  /// Return diagnostics for given file.
  GetDiagnostics(Vec<ModuleSpecifier>),
  /// Return document highlights at position.
  GetDocumentHighlights((ModuleSpecifier, u32, Vec<ModuleSpecifier>)),
  /// Get semantic highlights information for a particular file.
  GetEncodedSemanticClassifications((ModuleSpecifier, TextSpan)),
  /// Get implementation information for a specific position.
  GetImplementation((ModuleSpecifier, u32)),
  /// Get "navigate to" items, which are converted to workspace symbols
  GetNavigateToItems {
    search: String,
    max_result_count: Option<u32>,
    file: Option<String>,
  },
  /// Get a "navigation tree" for a specifier.
  GetNavigationTree(ModuleSpecifier),
  /// Get outlining spans for a specifier.
  GetOutliningSpans(ModuleSpecifier),
  /// Return quick info at position (hover information).
  GetQuickInfo((ModuleSpecifier, u32)),
  /// Get document references for a specific position.
  GetReferences((ModuleSpecifier, u32)),
  /// Get signature help items for a specific position.
  GetSignatureHelpItems((ModuleSpecifier, u32, SignatureHelpItemsOptions)),
  /// Get a selection range for a specific position.
  GetSmartSelectionRange((ModuleSpecifier, u32)),
  /// Get the diagnostic codes that support some form of code fix.
  GetSupportedCodeFixes,
  /// Get the type definition information for a specific position.
  GetTypeDefinition {
    specifier: ModuleSpecifier,
    position: u32,
  },
  /// Resolve a call hierarchy item for a specific position.
  PrepareCallHierarchy((ModuleSpecifier, u32)),
  /// Resolve incoming call hierarchy items for a specific position.
  ProvideCallHierarchyIncomingCalls((ModuleSpecifier, u32)),
  /// Resolve outgoing call hierarchy items for a specific position.
  ProvideCallHierarchyOutgoingCalls((ModuleSpecifier, u32)),
  /// Resolve inlay hints for a specific text span
  ProvideInlayHints((ModuleSpecifier, TextSpan, UserPreferences)),

  // Special request, used only internally by the LSP
  Restart,
}

impl RequestMethod {
  fn to_value(&self, state: &State, id: usize) -> Value {
    match self {
      RequestMethod::Configure(config) => json!({
        "id": id,
        "method": "configure",
        "compilerOptions": config,
      }),
      RequestMethod::FindRenameLocations {
        specifier,
        position,
        find_in_strings,
        find_in_comments,
        provide_prefix_and_suffix_text_for_rename,
      } => {
        json!({
          "id": id,
          "method": "findRenameLocations",
          "specifier": state.denormalize_specifier(specifier),
          "position": position,
          "findInStrings": find_in_strings,
          "findInComments": find_in_comments,
          "providePrefixAndSuffixTextForRename": provide_prefix_and_suffix_text_for_rename
        })
      }
      RequestMethod::GetAssets => json!({
        "id": id,
        "method": "getAssets",
      }),
      RequestMethod::GetApplicableRefactors((specifier, span, kind)) => json!({
        "id": id,
        "method": "getApplicableRefactors",
        "specifier": state.denormalize_specifier(specifier),
        "range": { "pos": span.start, "end": span.start + span.length },
        "kind": kind,
      }),
      RequestMethod::GetEditsForRefactor((
        specifier,
        span,
        refactor_name,
        action_name,
      )) => json!({
        "id": id,
        "method": "getEditsForRefactor",
        "specifier": state.denormalize_specifier(specifier),
        "range": { "pos": span.start, "end": span.start + span.length},
        "refactorName": refactor_name,
        "actionName": action_name,
      }),
      RequestMethod::GetCodeFixes((
        specifier,
        start_pos,
        end_pos,
        error_codes,
      )) => json!({
        "id": id,
        "method": "getCodeFixes",
        "specifier": state.denormalize_specifier(specifier),
        "startPosition": start_pos,
        "endPosition": end_pos,
        "errorCodes": error_codes,
      }),
      RequestMethod::GetCombinedCodeFix((specifier, fix_id)) => json!({
        "id": id,
        "method": "getCombinedCodeFix",
        "specifier": state.denormalize_specifier(specifier),
        "fixId": fix_id,
      }),
      RequestMethod::GetCompletionDetails(args) => json!({
        "id": id,
        "method": "getCompletionDetails",
        "args": args
      }),
      RequestMethod::GetCompletions((specifier, position, preferences)) => {
        json!({
          "id": id,
          "method": "getCompletions",
          "specifier": state.denormalize_specifier(specifier),
          "position": position,
          "preferences": preferences,
        })
      }
      RequestMethod::GetDefinition((specifier, position)) => json!({
        "id": id,
        "method": "getDefinition",
        "specifier": state.denormalize_specifier(specifier),
        "position": position,
      }),
      RequestMethod::GetDiagnostics(specifiers) => json!({
        "id": id,
        "method": "getDiagnostics",
        "specifiers": specifiers.iter().map(|s| state.denormalize_specifier(s)).collect::<Vec<String>>(),
      }),
      RequestMethod::GetDocumentHighlights((
        specifier,
        position,
        files_to_search,
      )) => json!({
        "id": id,
        "method": "getDocumentHighlights",
        "specifier": state.denormalize_specifier(specifier),
        "position": position,
        "filesToSearch": files_to_search,
      }),
      RequestMethod::GetEncodedSemanticClassifications((specifier, span)) => {
        json!({
          "id": id,
          "method": "getEncodedSemanticClassifications",
          "specifier": state.denormalize_specifier(specifier),
          "span": span,
        })
      }
      RequestMethod::GetImplementation((specifier, position)) => json!({
        "id": id,
        "method": "getImplementation",
        "specifier": state.denormalize_specifier(specifier),
        "position": position,
      }),
      RequestMethod::GetNavigateToItems {
        search,
        max_result_count,
        file,
      } => json!({
        "id": id,
        "method": "getNavigateToItems",
        "search": search,
        "maxResultCount": max_result_count,
        "file": file,
      }),
      RequestMethod::GetNavigationTree(specifier) => json!({
        "id": id,
        "method": "getNavigationTree",
        "specifier": state.denormalize_specifier(specifier),
      }),
      RequestMethod::GetOutliningSpans(specifier) => json!({
        "id": id,
        "method": "getOutliningSpans",
        "specifier": state.denormalize_specifier(specifier),
      }),
      RequestMethod::GetQuickInfo((specifier, position)) => json!({
        "id": id,
        "method": "getQuickInfo",
        "specifier": state.denormalize_specifier(specifier),
        "position": position,
      }),
      RequestMethod::GetReferences((specifier, position)) => json!({
        "id": id,
        "method": "getReferences",
        "specifier": state.denormalize_specifier(specifier),
        "position": position,
      }),
      RequestMethod::GetSignatureHelpItems((specifier, position, options)) => {
        json!({
          "id": id,
          "method": "getSignatureHelpItems",
          "specifier": state.denormalize_specifier(specifier),
          "position": position,
          "options": options,
        })
      }
      RequestMethod::GetSmartSelectionRange((specifier, position)) => {
        json!({
          "id": id,
          "method": "getSmartSelectionRange",
          "specifier": state.denormalize_specifier(specifier),
          "position": position
        })
      }
      RequestMethod::GetSupportedCodeFixes => json!({
        "id": id,
        "method": "getSupportedCodeFixes",
      }),
      RequestMethod::GetTypeDefinition {
        specifier,
        position,
      } => json!({
        "id": id,
        "method": "getTypeDefinition",
        "specifier": state.denormalize_specifier(specifier),
        "position": position
      }),
      RequestMethod::PrepareCallHierarchy((specifier, position)) => {
        json!({
          "id": id,
          "method": "prepareCallHierarchy",
          "specifier": state.denormalize_specifier(specifier),
          "position": position
        })
      }
      RequestMethod::ProvideCallHierarchyIncomingCalls((
        specifier,
        position,
      )) => {
        json!({
          "id": id,
          "method": "provideCallHierarchyIncomingCalls",
          "specifier": state.denormalize_specifier(specifier),
          "position": position
        })
      }
      RequestMethod::ProvideCallHierarchyOutgoingCalls((
        specifier,
        position,
      )) => {
        json!({
          "id": id,
          "method": "provideCallHierarchyOutgoingCalls",
          "specifier": state.denormalize_specifier(specifier),
          "position": position
        })
      }
      RequestMethod::ProvideInlayHints((specifier, span, preferences)) => {
        json!({
          "id": id,
          "method": "provideInlayHints",
          "specifier": state.denormalize_specifier(specifier),
          "span": span,
          "preferences": preferences,
        })
      }
      RequestMethod::Restart => json!({
        "id": id,
        "method": "restart",
      }),
    }
  }
}

/// Send a request into a runtime and return the JSON value of the response.
pub fn request(
  runtime: &mut JsRuntime,
  state_snapshot: Arc<StateSnapshot>,
  method: RequestMethod,
  token: CancellationToken,
) -> Result<Value, AnyError> {
  let (performance, request_params) = {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    let state = op_state.borrow_mut::<State>();
    state.state_snapshot = state_snapshot;
    state.token = token;
    state.last_id += 1;
    let id = state.last_id;
    (state.performance.clone(), method.to_value(state, id))
  };
  let mark = performance.mark("request", Some(request_params.clone()));
  let request_src = format!("globalThis.serverRequest({});", request_params);
  runtime.execute_script(&located_script_name!(), &request_src)?;

  let op_state = runtime.op_state();
  let mut op_state = op_state.borrow_mut();
  let state = op_state.borrow_mut::<State>();

  performance.measure(mark);
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
  use super::*;
  use crate::http_cache::HttpCache;
  use crate::http_util::HeadersMap;
  use crate::lsp::documents::Documents;
  use crate::lsp::documents::LanguageId;
  use crate::lsp::text::LineIndex;
  use std::path::Path;
  use std::path::PathBuf;
  use test_util::TempDir;

  fn mock_state_snapshot(
    fixtures: &[(&str, &str, i32, LanguageId)],
    location: &Path,
  ) -> StateSnapshot {
    let mut documents = Documents::new(location);
    for (specifier, source, version, language_id) in fixtures {
      let specifier =
        resolve_url(specifier).expect("failed to create specifier");
      documents.open(
        specifier.clone(),
        *version,
        language_id.clone(),
        (*source).into(),
      );
    }
    StateSnapshot {
      documents,
      ..Default::default()
    }
  }

  fn setup(
    temp_dir: &TempDir,
    debug: bool,
    config: Value,
    sources: &[(&str, &str, i32, LanguageId)],
  ) -> (JsRuntime, Arc<StateSnapshot>, PathBuf) {
    let location = temp_dir.path().join("deps");
    let state_snapshot = Arc::new(mock_state_snapshot(sources, &location));
    let mut runtime = js_runtime(Default::default());
    start(&mut runtime, debug, &state_snapshot)
      .expect("could not start server");
    let ts_config = TsConfig::new(config);
    assert_eq!(
      request(
        &mut runtime,
        state_snapshot.clone(),
        RequestMethod::Configure(ts_config),
        Default::default(),
      )
      .expect("failed request"),
      json!(true)
    );
    (runtime, state_snapshot, location)
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
    let temp_dir = TempDir::new();
    setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      &[],
    );
  }

  #[test]
  fn test_project_reconfigure() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      &[],
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
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!(true));
  }

  #[test]
  fn test_get_diagnostics() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      &[(
        "file:///a.ts",
        r#"console.log("hello deno");"#,
        1,
        LanguageId::TypeScript,
      )],
    );
    let specifier = resolve_url("file:///a.ts").expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!({
        "file:///a.ts": [
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
            "messageText": "Cannot find name 'console'. Do you need to change your target library? Try changing the \'lib\' compiler option to include 'dom'.",
            "sourceLine": "console.log(\"hello deno\");",
            "category": 1,
            "code": 2584
          }
        ]
      })
    );
  }

  #[test]
  fn test_get_diagnostics_lib() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "jsx": "react",
        "lib": ["esnext", "dom", "deno.ns"],
        "noEmit": true,
      }),
      &[(
        "file:///a.ts",
        r#"console.log(document.location);"#,
        1,
        LanguageId::TypeScript,
      )],
    );
    let specifier = resolve_url("file:///a.ts").expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!({ "file:///a.ts": [] }));
  }

  #[test]
  fn test_module_resolution() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "file:///a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
        LanguageId::TypeScript,
      )],
    );
    let specifier = resolve_url("file:///a.ts").expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!({ "file:///a.ts": [] }));
  }

  #[test]
  fn test_bad_module_specifiers() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "file:///a.ts",
        r#"
        import { A } from ".";
        "#,
        1,
        LanguageId::TypeScript,
      )],
    );
    let specifier = resolve_url("file:///a.ts").expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!({
        "file:///a.ts": [{
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
        }]
      })
    );
  }

  #[test]
  fn test_remote_modules() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "file:///a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
        LanguageId::TypeScript,
      )],
    );
    let specifier = resolve_url("file:///a.ts").expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!({ "file:///a.ts": [] }));
  }

  #[test]
  fn test_partial_modules() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
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
        LanguageId::TypeScript,
      )],
    );
    let specifier = resolve_url("file:///a.ts").expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!({
        "file:///a.ts": [{
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
        }]
      })
    );
  }

  #[test]
  fn test_no_debug_failure() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "file:///a.ts",
        r#"const url = new URL("b.js", import."#,
        1,
        LanguageId::TypeScript,
      )],
    );
    let specifier = resolve_url("file:///a.ts").expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!({
        "file:///a.ts": [
          {
            "start": {
              "line": 0,
              "character": 35,
            },
            "end": {
              "line": 0,
              "character": 35
            },
            "fileName": "file:///a.ts",
            "messageText": "Identifier expected.",
            "sourceLine": "const url = new URL(\"b.js\", import.",
            "category": 1,
            "code": 1003,
          }
        ]
      })
    );
  }

  #[test]
  fn test_request_assets() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) =
      setup(&temp_dir, false, json!({}), &[]);
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetAssets,
      Default::default(),
    )
    .unwrap();
    let assets = result.as_array().unwrap();

    // You might have found this assertion starts failing after upgrading TypeScript.
    // Just update the new number of assets (declaration files) for this number.
    assert_eq!(assets.len(), 71);

    // get some notification when the size of the assets grows
    let mut total_size = 0;
    for asset in assets {
      let obj = asset.as_object().unwrap();
      let text = obj.get("text").unwrap().as_str().unwrap();
      total_size += text.len();
    }
    assert!(total_size > 0);
    assert!(total_size < 2_000_000); // currently as of TS 4.6, it's 0.7MB
  }

  #[test]
  fn test_modify_sources() {
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, location) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "file:///a.ts",
        r#"
          import * as a from "https://deno.land/x/example/a.ts";
          if (a.a === "b") {
            console.log("fail");
          }
        "#,
        1,
        LanguageId::TypeScript,
      )],
    );
    let cache = HttpCache::new(&location);
    let specifier_dep =
      resolve_url("https://deno.land/x/example/a.ts").unwrap();
    cache
      .set(
        &specifier_dep,
        HeadersMap::default(),
        b"export const b = \"b\";\n",
      )
      .unwrap();
    let specifier = resolve_url("file:///a.ts").unwrap();
    let result = request(
      &mut runtime,
      state_snapshot.clone(),
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!({
        "file:///a.ts": [
          {
            "start": {
              "line": 2,
              "character": 16,
            },
            "end": {
              "line": 2,
              "character": 17
            },
            "fileName": "file:///a.ts",
            "messageText": "Property \'a\' does not exist on type \'typeof import(\"https://deno.land/x/example/a\")\'.",
            "sourceLine": "          if (a.a === \"b\") {",
            "code": 2339,
            "category": 1,
          }
        ]
      })
    );
    cache
      .set(
        &specifier_dep,
        HeadersMap::default(),
        b"export const b = \"b\";\n\nexport const a = \"b\";\n",
      )
      .unwrap();
    let specifier = resolve_url("file:///a.ts").unwrap();
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetDiagnostics(vec![specifier]),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!({
        "file:///a.ts": []
      })
    );
  }

  #[test]
  fn test_op_exists() {
    let temp_dir = TempDir::new();
    let (mut rt, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[],
    );
    let performance = Arc::new(Performance::default());
    let state = State::new(state_snapshot, performance);
    let op_state = rt.op_state();
    let mut op_state = op_state.borrow_mut();
    op_state.put(state);
    let actual = op_exists::call(
      &mut op_state,
      SpecifierArgs {
        specifier: "/error/unknown:something/index.d.ts".to_string(),
      },
    );
    assert!(!actual);
  }

  #[test]
  fn test_completion_entry_filter_text() {
    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "['foo']".to_string(),
      insert_text: Some("['foo']".to_string()),
      ..Default::default()
    };
    let actual = fixture.get_filter_text();
    assert_eq!(actual, Some(".foo".to_string()));

    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "#abc".to_string(),
      ..Default::default()
    };
    let actual = fixture.get_filter_text();
    assert_eq!(actual, Some("abc".to_string()));

    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "#abc".to_string(),
      insert_text: Some("this.#abc".to_string()),
      ..Default::default()
    };
    let actual = fixture.get_filter_text();
    assert_eq!(actual, Some("abc".to_string()));
  }

  #[test]
  fn test_completions() {
    let fixture = r#"
      import { B } from "https://deno.land/x/b/mod.ts";

      const b = new B();

      console.
    "#;
    let line_index = LineIndex::new(fixture);
    let position = line_index
      .offset_tsc(lsp::Position {
        line: 5,
        character: 16,
      })
      .unwrap();
    let temp_dir = TempDir::new();
    let (mut runtime, state_snapshot, _) = setup(
      &temp_dir,
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[("file:///a.ts", fixture, 1, LanguageId::TypeScript)],
    );
    let specifier = resolve_url("file:///a.ts").expect("could not resolve url");
    let result = request(
      &mut runtime,
      state_snapshot.clone(),
      RequestMethod::GetDiagnostics(vec![specifier.clone()]),
      Default::default(),
    );
    assert!(result.is_ok());
    let result = request(
      &mut runtime,
      state_snapshot.clone(),
      RequestMethod::GetCompletions((
        specifier.clone(),
        position,
        GetCompletionsAtPositionOptions {
          user_preferences: UserPreferences {
            include_completions_with_insert_text: Some(true),
            ..Default::default()
          },
          trigger_character: Some(".".to_string()),
          trigger_kind: None,
        },
      )),
      Default::default(),
    );
    assert!(result.is_ok());
    let response: CompletionInfo =
      serde_json::from_value(result.unwrap()).unwrap();
    assert_eq!(response.entries.len(), 19);
    let result = request(
      &mut runtime,
      state_snapshot,
      RequestMethod::GetCompletionDetails(GetCompletionDetailsArgs {
        specifier,
        position,
        name: "log".to_string(),
        source: None,
        preferences: None,
        data: None,
      }),
      Default::default(),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!({
        "name": "log",
        "kindModifiers": "declare",
        "kind": "method",
        "displayParts": [
          {
            "text": "(",
            "kind": "punctuation"
          },
          {
            "text": "method",
            "kind": "text"
          },
          {
            "text": ")",
            "kind": "punctuation"
          },
          {
            "text": " ",
            "kind": "space"
          },
          {
            "text": "Console",
            "kind": "interfaceName"
          },
          {
            "text": ".",
            "kind": "punctuation"
          },
          {
            "text": "log",
            "kind": "methodName"
          },
          {
            "text": "(",
            "kind": "punctuation"
          },
          {
            "text": "...",
            "kind": "punctuation"
          },
          {
            "text": "data",
            "kind": "parameterName"
          },
          {
            "text": ":",
            "kind": "punctuation"
          },
          {
            "text": " ",
            "kind": "space"
          },
          {
            "text": "any",
            "kind": "keyword"
          },
          {
            "text": "[",
            "kind": "punctuation"
          },
          {
            "text": "]",
            "kind": "punctuation"
          },
          {
            "text": ")",
            "kind": "punctuation"
          },
          {
            "text": ":",
            "kind": "punctuation"
          },
          {
            "text": " ",
            "kind": "space"
          },
          {
            "text": "void",
            "kind": "keyword"
          }
        ],
        "documentation": []
      })
    );
  }

  #[test]
  fn test_update_import_statement() {
    let fixtures = vec![
      (
        "file:///a/a.ts",
        "./b",
        "file:///a/b.ts",
        "import { b } from \"./b\";\n\n",
        "import { b } from \"./b.ts\";\n\n",
      ),
      (
        "file:///a/a.ts",
        "../b/b",
        "file:///b/b.ts",
        "import { b } from \"../b/b\";\n\n",
        "import { b } from \"../b/b.ts\";\n\n",
      ),
      ("file:///a/a.ts", "./b", "file:///a/b.ts", ", b", ", b"),
    ];

    for (
      specifier_text,
      module_specifier,
      file_name,
      orig_text,
      expected_text,
    ) in fixtures
    {
      let specifier = ModuleSpecifier::parse(specifier_text).unwrap();
      let item_data = CompletionItemData {
        specifier: specifier.clone(),
        position: 0,
        name: "b".to_string(),
        source: None,
        data: Some(json!({
          "moduleSpecifier": module_specifier,
          "fileName": file_name,
        })),
        use_code_snippet: false,
      };
      let actual = update_import_statement(
        lsp::TextEdit {
          range: lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 0,
            },
            end: lsp::Position {
              line: 0,
              character: 0,
            },
          },
          new_text: orig_text.to_string(),
        },
        &item_data,
      );
      assert_eq!(
        actual,
        lsp::TextEdit {
          range: lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 0,
            },
            end: lsp::Position {
              line: 0,
              character: 0,
            },
          },
          new_text: expected_text.to_string(),
        }
      );
    }
  }
}
