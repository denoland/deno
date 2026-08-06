// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::convert::Infallible;
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
use lsp_types::Uri;
use text_size::TextSize;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

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

#[derive(Serialize, Clone, Copy)]
#[allow(dead_code, reason = "currently unused")]
pub struct JsNull;
