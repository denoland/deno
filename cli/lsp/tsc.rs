// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::convert::Infallible;
use std::ops::Range;
use std::sync::Arc;

use dashmap::DashMap;
use deno_core::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::convert::Smi;
use deno_core::convert::ToV8;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::v8;
use lazy_regex::lazy_regex;
use lsp_types::Uri;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_repr::Deserialize_repr;
use serde_repr::Serialize_repr;
use text_size::TextRange;
use text_size::TextSize;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::code_lens;
use super::code_lens::CodeLensData;
use super::config;
use super::documents::DocumentModule;
use super::language_server;
use super::language_server::StateSnapshot;
use super::text::LineIndex;
use super::urls::url_to_uri;


#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum ChangeKind {
  Opened = 0,
  Modified = 1,
  Closed = 2,
}

impl<'a> ToV8<'a> for ChangeKind {
  type Error = Infallible;
  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    Smi(self as u8).to_v8(scope)
  }
}

impl Serialize for ChangeKind {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_i32(*self as i32)
  }
}


/// Aligns with ts.ScriptElementKind
#[derive(
  Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq, Hash,
)]
pub enum ScriptElementKind {
  #[serde(rename = "")]
  #[default]
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
  #[serde(rename = "using")]
  VariableUsingElement,
  #[serde(rename = "await using")]
  VariableAwaitUsingElement,
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
  #[serde(rename = "accessor")]
  MemberAccessorVariableElement,
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

/// This mirrors the method `convertKind` in `completions.ts` in vscode (extensions/typescript-language-features)
/// https://github.com/microsoft/vscode/blob/bd2df940d74b51105aefb11304e028d2fb56a9dc/extensions/typescript-language-features/src/languageFeatures/completions.ts#L440
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
      ScriptElementKind::LocalClassElement
      | ScriptElementKind::ConstructorImplementationElement
      | ScriptElementKind::TypeParameterElement
      | ScriptElementKind::Label
      | ScriptElementKind::JsxAttribute
      | ScriptElementKind::Link
      | ScriptElementKind::LinkName
      | ScriptElementKind::LinkText
      | ScriptElementKind::VariableUsingElement
      | ScriptElementKind::VariableAwaitUsingElement
      | ScriptElementKind::MemberAccessorVariableElement
      | ScriptElementKind::Unknown => lsp::CompletionItemKind::PROPERTY,
    }
  }
}

static PART_KIND_MODIFIER_RE: Lazy<Regex> = lazy_regex!(r",|\s+");

fn parse_kind_modifier(kind_modifiers: &str) -> HashSet<&str> {
  PART_KIND_MODIFIER_RE.split(kind_modifiers).collect()
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TextSpan {
  pub start: u32,
  pub length: u32,
}

impl TextSpan {
  pub fn to_range(&self, line_index: &LineIndex) -> lsp::Range {
    lsp::Range {
      start: line_index.position_utf16(self.start.into()),
      end: line_index.position_utf16(TextSize::from(self.start + self.length)),
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
    line_index: &LineIndex,
    uri: &Uri,
    source: code_lens::CodeLensSource,
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
      data: Some(json!(CodeLensData {
        source,
        uri: uri.clone(),
      })),
    }
  }

  pub fn collect_document_symbols(
    &self,
    line_index: &LineIndex,
    document_symbols: &mut Vec<lsp::DocumentSymbol>,
  ) -> bool {
    let mut should_include = self.should_include_entry();
    if !should_include
      && self
        .child_items
        .as_ref()
        .map(|v| v.is_empty())
        .unwrap_or(true)
    {
      return false;
    }

    let children = self
      .child_items
      .as_deref()
      .unwrap_or(&[] as &[NavigationTree]);
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
          let included_child =
            child.collect_document_symbols(line_index, &mut symbol_children);
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
        // a default, therefore we have to supply the deprecated
        // field. It is like a bad version of Inception.
        #[allow(deprecated, reason = "see comment")]
        document_symbols.push(lsp::DocumentSymbol {
          name,
          kind: self.kind.clone().into(),
          range: span.to_range(line_index),
          selection_range: selection_span.to_range(line_index),
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

  pub fn walk<F>(
    &self,
    token: &CancellationToken,
    callback: &F,
  ) -> Result<(), AnyError>
  where
    F: Fn(&NavigationTree, Option<&NavigationTree>),
  {
    callback(self, None);
    if let Some(child_items) = &self.child_items {
      for child in child_items {
        if token.is_cancelled() {
          return Err(anyhow!("request cancelled"));
        }
        child.walk_child(token, callback, self)?;
      }
    }
    Ok(())
  }

  fn walk_child<F>(
    &self,
    token: &CancellationToken,
    callback: &F,
    parent: &NavigationTree,
  ) -> Result<(), AnyError>
  where
    F: Fn(&NavigationTree, Option<&NavigationTree>),
  {
    callback(self, Some(parent));
    if let Some(child_items) = &self.child_items {
      for child in child_items {
        if token.is_cancelled() {
          return Err(anyhow!("request cancelled"));
        }
        child.walk_child(token, callback, self)?;
      }
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TextChange {
  pub span: TextSpan,
  pub new_text: String,
}

impl TextChange {
  pub fn as_text_edit(&self, line_index: &LineIndex) -> lsp::TextEdit {
    lsp::TextEdit {
      range: self.span.to_range(line_index),
      new_text: self.new_text.clone(),
    }
  }

  pub fn as_text_or_annotated_text_edit(
    &self,
    line_index: &LineIndex,
  ) -> lsp::OneOf<lsp::TextEdit, lsp::AnnotatedTextEdit> {
    lsp::OneOf::Left(lsp::TextEdit {
      range: self.span.to_range(line_index),
      new_text: self.new_text.clone(),
    })
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct FileTextChanges {
  pub file_name: String,
  pub text_changes: Vec<TextChange>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub is_new_file: Option<bool>,
}

impl FileTextChanges {
  pub fn to_text_edits(
    &self,
    module: &DocumentModule,
    language_server: &language_server::Inner,
  ) -> Option<(Uri, Vec<lsp::TextEdit>)> {
    let is_new_file = self.is_new_file.unwrap_or(false);
    let target_specifier = resolve_url(&self.file_name).ok()?;
    let target_module = if is_new_file {
      None
    } else {
      Some(language_server.document_modules.module_for_specifier(
        &target_specifier,
        module.scope.as_deref(),
        Some(&module.compiler_options_key),
      )?)
    };
    let target_uri = target_module
      .as_ref()
      .map(|m| m.uri.clone())
      .or_else(|| url_to_uri(&target_specifier).ok().map(Arc::new))?;
    let line_index = target_module
      .as_ref()
      .map(|m| Cow::Borrowed(m.line_index.as_ref()))
      .unwrap_or_else(|| Cow::Owned(LineIndex::new("")));
    let edits = self
      .text_changes
      .iter()
      .map(|tc| tc.as_text_edit(&line_index))
      .collect();
    Some((target_uri.as_ref().clone(), edits))
  }

  pub fn to_text_document_change_ops(
    &self,
    module: &DocumentModule,
    snapshot: &StateSnapshot,
  ) -> Option<Vec<lsp::DocumentChangeOperation>> {
    let is_new_file = self.is_new_file.unwrap_or(false);
    let mut ops = Vec::<lsp::DocumentChangeOperation>::new();
    let target_specifier = resolve_url(&self.file_name).ok()?;
    let target_module = if is_new_file {
      None
    } else {
      Some(snapshot.document_modules.module_for_specifier(
        &target_specifier,
        module.scope.as_deref(),
        Some(&module.compiler_options_key),
      )?)
    };
    let target_uri = target_module
      .as_ref()
      .map(|m| m.uri.clone())
      .or_else(|| url_to_uri(&target_specifier).ok().map(Arc::new))?;
    let line_index = target_module
      .as_ref()
      .map(|m| Cow::Borrowed(m.line_index.as_ref()))
      .unwrap_or_else(|| Cow::Owned(LineIndex::new("")));

    if is_new_file {
      ops.push(lsp::DocumentChangeOperation::Op(lsp::ResourceOp::Create(
        lsp::CreateFile {
          uri: target_uri.as_ref().clone(),
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
      .map(|tc| tc.as_text_or_annotated_text_edit(&line_index))
      .collect();
    ops.push(lsp::DocumentChangeOperation::Edit(lsp::TextDocumentEdit {
      text_document: lsp::OptionalVersionedTextDocumentIdentifier {
        uri: target_uri.as_ref().clone(),
        version: target_module
          .as_ref()
          .and_then(|m| m.open_data.as_ref())
          .map(|d| d.version),
      },
      edits,
    }));

    Some(ops)
  }
}

pub fn file_text_changes_to_workspace_edit<'a>(
  changes_with_modules: impl IntoIterator<
    Item = (&'a FileTextChanges, &'a DocumentModule),
  >,
  snapshot: &StateSnapshot,
  token: &CancellationToken,
) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
  let mut all_ops = Vec::<lsp::DocumentChangeOperation>::new();
  for (change, module) in changes_with_modules {
    if token.is_cancelled() {
      return Err(anyhow!("request cancelled"));
    }
    let Some(ops) = change.to_text_document_change_ops(module, snapshot) else {
      continue;
    };
    all_ops.extend(ops);
  }
  if all_ops.is_empty() {
    return Ok(None);
  }
  Ok(Some(lsp::WorkspaceEdit {
    document_changes: Some(lsp::DocumentChanges::Operations(all_ops)),
    ..Default::default()
  }))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefactorEditInfo {
  pub edits: Vec<FileTextChanges>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rename_filename: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rename_location: Option<u32>,
}

impl RefactorEditInfo {
  pub fn to_workspace_edit(
    &self,
    module: &Arc<DocumentModule>,
    snapshot: &StateSnapshot,
    token: &CancellationToken,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    file_text_changes_to_workspace_edit(
      self.edits.iter().map(|c| (c, module.as_ref())),
      snapshot,
      token,
    )
  }

  pub fn to_rename_command(
    &self,
    module: &Arc<DocumentModule>,
    snapshot: &StateSnapshot,
  ) -> Option<lsp::Command> {
    let rename_location = self.rename_location?;
    let rename_filename = self.rename_filename.as_ref()?;
    let target_specifier = resolve_url(rename_filename).ok()?;
    let target_module = snapshot.document_modules.module_for_specifier(
      &target_specifier,
      module.scope.as_deref(),
      Some(&module.compiler_options_key),
    )?;
    let changes = self
      .edits
      .iter()
      .find(|c| &c.file_name == rename_filename)?;
    let mut text = target_module.text.to_string();
    for change in changes.text_changes.iter().rev() {
      let range = change.span.to_range(&target_module.line_index);
      let start = target_module.line_index.offset(range.start).ok()?;
      let end = target_module.line_index.offset(range.end).ok()?;
      text.replace_range(
        u32::from(start) as usize..u32::from(end) as usize,
        &change.new_text,
      );
    }
    Some(lsp::Command {
      title: "".to_string(),
      command: "editor.action.rename".to_string(),
      arguments: Some(vec![json!([
        target_module.uri.as_ref(),
        LineIndex::new(&text).position_utf16(rename_location.into())
      ])]),
    })
  }
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletionSpecifierRewrite {
  new_specifier: String,
  new_deno_types_specifier: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsJsCompletionItemData {
  pub uri: Uri,
  pub position: u32,
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  /// If present, the code action / text edit corresponding to this item should
  /// be rewritten by replacing the first string with the second. Intended for
  /// auto-import specifiers to be reverse-import-mapped.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub specifier_rewrite: Option<CompletionSpecifierRewrite>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<Value>,
  pub use_code_snippet: bool,
}

#[derive(Debug, Default)]
pub struct TscSpecifierMap {
  normalized_specifiers: DashMap<String, ModuleSpecifier>,
}

impl TscSpecifierMap {
  pub fn new() -> Self {
    Self::default()
  }

  /// Convert the specifier from one compatible with tsc. Cache the resulting
  /// mapping in case it needs to be reversed.
  pub fn normalize<S: AsRef<str>>(
    &self,
    specifier: S,
  ) -> Result<ModuleSpecifier, deno_core::url::ParseError> {
    let original = specifier.as_ref();
    if let Some(specifier) = self.normalized_specifiers.get(original) {
      return Ok(specifier.clone());
    }
    let specifier_str = original
      .replace(".d.ts.d.ts", ".d.ts")
      .replace("$node_modules", "node_modules");
    let specifier = ModuleSpecifier::parse(&specifier_str)?;
    Ok(specifier)
  }
}


#[derive(Debug, Clone, Deserialize_repr, Serialize_repr)]
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

pub type QuotePreference = config::QuoteStyle;

pub type ImportModuleSpecifierPreference = config::ImportModuleSpecifier;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code, reason = "unsupported")]
pub enum ImportModuleSpecifierEnding {
  Auto,
  Minimal,
  Index,
  Js,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code, reason = "unsupported")]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code, reason = "unsupported")]
pub enum IncludePackageJsonAutoImports {
  Auto,
  On,
  Off,
}

pub type JsxAttributeCompletionStyle = config::JsxAttributeCompletionStyle;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GetCompletionsAtPositionOptions {
  #[serde(flatten)]
  pub user_preferences: UserPreferences,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_character: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_kind: Option<CompletionTriggerKind>,
}

#[derive(Debug, Default, Clone, Serialize)]
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub interactive_inlay_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub prefer_type_only_auto_imports: Option<bool>,
}

impl UserPreferences {
  pub fn from_config_for_specifier(
    config: &config::Config,
    specifier: &ModuleSpecifier,
  ) -> Self {
    let fmt_options = config.tree.fmt_config_for_specifier(specifier);
    let fmt_config = &fmt_options.options;
    let base_preferences = Self {
      allow_incomplete_completions: Some(true),
      allow_text_changes_in_new_files: Some(specifier.scheme() == "file"),
      // TODO(nayeemrmn): Investigate why we use `Index` here.
      import_module_specifier_ending: Some(ImportModuleSpecifierEnding::Index),
      include_completions_with_snippet_text: Some(
        config.snippet_support_capable(),
      ),
      interactive_inlay_hints: Some(true),
      provide_refactor_not_applicable_reason: Some(true),
      quote_preference: Some(fmt_config.into()),
      use_label_details_in_completion_entries: Some(true),
      ..Default::default()
    };
    let Some(language_settings) =
      config.language_settings_for_specifier(specifier)
    else {
      return base_preferences;
    };
    Self {
      auto_import_file_exclude_patterns: Some(
        language_settings
          .preferences
          .auto_import_file_exclude_patterns
          .clone(),
      ),
      include_automatic_optional_chain_completions: Some(
        language_settings.suggest.enabled
          && language_settings
            .suggest
            .include_automatic_optional_chain_completions,
      ),
      include_completions_for_import_statements: Some(
        language_settings.suggest.enabled
          && language_settings
            .suggest
            .include_completions_for_import_statements,
      ),
      include_completions_for_module_exports: Some(
        language_settings.suggest.enabled
          && language_settings.suggest.auto_imports,
      ),
      include_completions_with_class_member_snippets: Some(
        language_settings.suggest.enabled
          && language_settings.suggest.class_member_snippets.enabled
          && config.snippet_support_capable(),
      ),
      include_completions_with_insert_text: Some(
        language_settings.suggest.enabled,
      ),
      include_completions_with_object_literal_method_snippets: Some(
        language_settings.suggest.enabled
          && language_settings
            .suggest
            .object_literal_method_snippets
            .enabled
          && config.snippet_support_capable(),
      ),
      import_module_specifier_preference: Some(
        language_settings.preferences.import_module_specifier,
      ),
      include_inlay_parameter_name_hints: Some(
        (&language_settings.inlay_hints.parameter_names.enabled).into(),
      ),
      include_inlay_parameter_name_hints_when_argument_matches_name: Some(
        !language_settings
          .inlay_hints
          .parameter_names
          .suppress_when_argument_matches_name,
      ),
      include_inlay_function_parameter_type_hints: Some(
        language_settings.inlay_hints.parameter_types.enabled,
      ),
      include_inlay_variable_type_hints: Some(
        language_settings.inlay_hints.variable_types.enabled,
      ),
      include_inlay_variable_type_hints_when_type_matches_name: Some(
        !language_settings
          .inlay_hints
          .variable_types
          .suppress_when_type_matches_name,
      ),
      include_inlay_property_declaration_type_hints: Some(
        language_settings
          .inlay_hints
          .property_declaration_types
          .enabled,
      ),
      include_inlay_function_like_return_type_hints: Some(
        language_settings
          .inlay_hints
          .function_like_return_types
          .enabled,
      ),
      include_inlay_enum_member_value_hints: Some(
        language_settings.inlay_hints.enum_member_values.enabled,
      ),
      jsx_attribute_completion_style: Some(
        language_settings.preferences.jsx_attribute_completion_style,
      ),
      provide_prefix_and_suffix_text_for_rename: Some(
        language_settings.preferences.use_aliases_for_renames,
      ),
      // Only use workspace settings for quote style if there's no `deno.json`.
      quote_preference: if config
        .tree
        .workspace_dir_for_specifier(specifier)
        .is_some_and(|ctx| ctx.member_or_root_deno_json().is_some())
      {
        base_preferences.quote_preference
      } else {
        Some(language_settings.preferences.quote_style)
      },
      prefer_type_only_auto_imports: Some(
        language_settings.preferences.prefer_type_only_auto_imports,
      ),
      ..base_preferences
    }
  }
}

// `Serialize` is retained because the whole `TscRequest` is serialized to a
// JSON performance-trace via `Performance::mark_with_args`; `ToV8` is what
// actually crosses the V8 boundary in `into_server_request`.
#[derive(Debug, Clone, Serialize, deno_core::ToV8)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpItemsOptions {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[to_v8(skip_if = Option::is_none)]
  pub trigger_reason: Option<SignatureHelpTriggerReason>,
}

#[derive(Debug, Clone, Serialize, deno_core::ToV8)]
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

#[derive(Debug, Clone, Serialize, deno_core::ToV8)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpTriggerReason {
  pub kind: SignatureHelpTriggerKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[to_v8(skip_if = Option::is_none)]
  pub trigger_character: Option<String>,
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct TscTextRange {
  pos: u32,
  end: u32,
}

impl From<Range<u32>> for TscTextRange {
  fn from(range: Range<u32>) -> Self {
    Self {
      pos: range.start,
      end: range.end,
    }
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CombinedCodeFixScope {
  r#type: &'static str,
  file_name: String,
}

#[derive(Debug, Serialize, Clone)]
pub enum OrganizeImportsMode {
  All,
  #[allow(unused, reason = "unsupported")]
  SortAndCombine,
  #[allow(unused, reason = "unsupported")]
  RemoveUnused,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeImportsArgs {
  #[serde(flatten)]
  pub scope: CombinedCodeFixScope,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mode: Option<OrganizeImportsMode>,
}

#[derive(Serialize, Clone, Copy)]
#[allow(dead_code, reason = "currently unused")]
pub struct JsNull;
