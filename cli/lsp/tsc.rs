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

      Option<String>,
      Option<UserPreferences>,
      Option<Value>,
    )>,
  ),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6228
  GetImplementationAtPosition((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6240
  GetOutliningSpans((String,)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6237
  ProvideCallHierarchyIncomingCalls((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6238
  ProvideCallHierarchyOutgoingCalls((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6236
  PrepareCallHierarchy((String, u32)),
  // https://github.com/denoland/deno/blob/v2.2.2/cli/tsc/dts/typescript.d.ts#L6674
  FindRenameLocations((String, u32, bool, bool, UserPreferences)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6224
  GetSmartSelectionRange((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6183
  GetEncodedSemanticClassifications((String, TextSpan, &'static str)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6217
  GetSignatureHelpItems((String, u32, SignatureHelpItemsOptions)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6233
  GetNavigateToItems((String, Option<u32>, Option<String>)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6239
  ProvideInlayHints((String, TextSpan, UserPreferences)),
  // https://github.com/denoland/deno/blob/v2.5.2/cli/tsc/dts/typescript.d.ts#L6769
  OrganizeImports(
    (
      OrganizeImportsArgs,
      FormatCodeSettings,
      Option<UserPreferences>,
    ),
  ),
}

impl TscRequest {
  /// Converts the request into a tuple containing the method name and the
  /// arguments (in the form of a V8 value) to be passed to the server request
  /// function
  fn into_server_request<'s>(
    self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> Result<(&'static str, Option<v8::Local<'s, v8::Value>>), JsErrorBox> {
    // Requests whose arguments are positional tuples of primitives serialize to
    // a `v8::Array` via the `ToV8` derives/impls (matching serde's tuple
    // encoding). Bare integers are wrapped in `Number` so they map to a JS
    // `number` exactly as serde did. Requests carrying richer config structs
    // (`UserPreferences`, `FormatCodeSettings`, …) or `#[serde(flatten)]` stay
    // on `serde_v8::to_v8` until those types grow `ToV8` impls.
    let args = match self {
      TscRequest::GetDiagnostics(args) => (
        "$getDiagnostics",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetAmbientModules => ("$getAmbientModules", None),
      TscRequest::FindReferences((specifier, position)) => (
        "findReferences",
        Some((specifier, Number(position)).to_v8(scope)?),
      ),
      TscRequest::GetNavigationTree(args) => {
        ("getNavigationTree", Some(args.to_v8(scope)?))
      }
      TscRequest::GetSupportedCodeFixes => ("$getSupportedCodeFixes", None),
      TscRequest::GetQuickInfoAtPosition((
        specifier,
        position,
        maximum_length,
      )) => (
        "getQuickInfoAtPosition",
        Some(
          (specifier, Number(position), Number(maximum_length)).to_v8(scope)?,
        ),
      ),
      TscRequest::GetCodeFixesAtPosition(args) => (
        "getCodeFixesAtPosition",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetApplicableRefactors(args) => (
        "getApplicableRefactors",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetCombinedCodeFix(args) => (
        "getCombinedCodeFix",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetEditsForRefactor(args) => (
        "getEditsForRefactor",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetEditsForFileRename(args) => (
        "getEditsForFileRename",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetDocumentHighlights(args) => {
        let (specifier, position, files_to_search) = *args;
        (
          "getDocumentHighlights",
          Some((specifier, Number(position), files_to_search).to_v8(scope)?),
        )
      }
      TscRequest::GetDefinitionAndBoundSpan((specifier, position)) => (
        "getDefinitionAndBoundSpan",
        Some((specifier, Number(position)).to_v8(scope)?),
      ),
      TscRequest::GetTypeDefinitionAtPosition((specifier, position)) => (
        "getTypeDefinitionAtPosition",
        Some((specifier, Number(position)).to_v8(scope)?),
      ),
      TscRequest::GetCompletionsAtPosition(args) => (
        "getCompletionsAtPosition",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetCompletionEntryDetails(args) => (
        "getCompletionEntryDetails",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetImplementationAtPosition((specifier, position)) => (
        "getImplementationAtPosition",
        Some((specifier, Number(position)).to_v8(scope)?),
      ),
      TscRequest::GetOutliningSpans(args) => {
        ("getOutliningSpans", Some(args.to_v8(scope)?))
      }
      TscRequest::ProvideCallHierarchyIncomingCalls((specifier, position)) => (
        "provideCallHierarchyIncomingCalls",
        Some((specifier, Number(position)).to_v8(scope)?),
      ),
      TscRequest::ProvideCallHierarchyOutgoingCalls((specifier, position)) => (
        "provideCallHierarchyOutgoingCalls",
        Some((specifier, Number(position)).to_v8(scope)?),
      ),
      TscRequest::PrepareCallHierarchy((specifier, position)) => (
        "prepareCallHierarchy",
        Some((specifier, Number(position)).to_v8(scope)?),
      ),
      TscRequest::FindRenameLocations(args) => (
        "findRenameLocations",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetSmartSelectionRange((specifier, position)) => (
        "getSmartSelectionRange",
        Some((specifier, Number(position)).to_v8(scope)?),
      ),
      TscRequest::GetEncodedSemanticClassifications(args) => (
        "getEncodedSemanticClassifications",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::GetSignatureHelpItems((specifier, position, options)) => (
        "getSignatureHelpItems",
        Some((specifier, Number(position), options).to_v8(scope)?),
      ),
      TscRequest::GetNavigateToItems((search, max_result_count, file)) => (
        "getNavigateToItems",
        Some((search, max_result_count.map(Number), file).to_v8(scope)?),
      ),
      TscRequest::ProvideInlayHints(args) => (
        "provideInlayHints",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::OrganizeImports(args) => (
        "organizeImports",
        Some(serde_v8::to_v8(scope, args).map_err(JsErrorBox::from_err)?),
      ),
      TscRequest::CleanupSemanticCache => ("$cleanupSemanticCache", None),
      TscRequest::ReleaseMemory => ("$releaseMemory", None),
    };

    Ok(args)
  }

  fn method(&self) -> &'static str {
    match self {
      TscRequest::GetDiagnostics(_) => "$getDiagnostics",
      TscRequest::GetAmbientModules => "$getAmbientModules",
      TscRequest::CleanupSemanticCache => "$cleanupSemanticCache",
      TscRequest::ReleaseMemory => "$releaseMemory",
      TscRequest::FindReferences(_) => "findReferences",
      TscRequest::GetNavigationTree(_) => "getNavigationTree",
      TscRequest::GetSupportedCodeFixes => "$getSupportedCodeFixes",
      TscRequest::GetQuickInfoAtPosition(_) => "getQuickInfoAtPosition",
      TscRequest::GetCodeFixesAtPosition(_) => "getCodeFixesAtPosition",
      TscRequest::GetApplicableRefactors(_) => "getApplicableRefactors",
      TscRequest::GetCombinedCodeFix(_) => "getCombinedCodeFix",
      TscRequest::GetEditsForRefactor(_) => "getEditsForRefactor",
      TscRequest::GetEditsForFileRename(_) => "getEditsForFileRename",
      TscRequest::GetDocumentHighlights(_) => "getDocumentHighlights",
      TscRequest::GetDefinitionAndBoundSpan(_) => "getDefinitionAndBoundSpan",
      TscRequest::GetTypeDefinitionAtPosition(_) => {
        "getTypeDefinitionAtPosition"
      }
      TscRequest::GetCompletionsAtPosition(_) => "getCompletionsAtPosition",
      TscRequest::GetCompletionEntryDetails(_) => "getCompletionEntryDetails",
      TscRequest::GetImplementationAtPosition(_) => {
        "getImplementationAtPosition"
      }
      TscRequest::GetOutliningSpans(_) => "getOutliningSpans",
      TscRequest::ProvideCallHierarchyIncomingCalls(_) => {
        "provideCallHierarchyIncomingCalls"
      }
      TscRequest::ProvideCallHierarchyOutgoingCalls(_) => {
        "provideCallHierarchyOutgoingCalls"
      }
      TscRequest::PrepareCallHierarchy(_) => "prepareCallHierarchy",
      TscRequest::FindRenameLocations(_) => "findRenameLocations",
      TscRequest::GetSmartSelectionRange(_) => "getSmartSelectionRange",
      TscRequest::GetEncodedSemanticClassifications(_) => {
        "getEncodedSemanticClassifications"
      }
      TscRequest::GetSignatureHelpItems(_) => "getSignatureHelpItems",
      TscRequest::GetNavigateToItems(_) => "getNavigateToItems",
      TscRequest::ProvideInlayHints(_) => "provideInlayHints",
      TscRequest::OrganizeImports(_) => "organizeImports",
    }
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  use super::*;
  use crate::cache::HttpCache;
  use crate::lsp::cache::LspCache;
  use crate::lsp::compiler_options::LspCompilerOptionsResolver;
  use crate::lsp::config::Config;
  use crate::lsp::config::WorkspaceSettings;
  use crate::lsp::documents::DocumentModules;
  use crate::lsp::documents::LanguageId;
  use crate::lsp::lint::LspLinterResolver;
  use crate::lsp::resolver::LspResolver;
  use crate::lsp::text::LineIndex;

  #[test]
  fn test_find_mergeable_named_import() {
    let new_import =
      parse_simple_named_import("import { think } from \"cowsay\";\n\n")
        .unwrap();
    let existing_import = find_mergeable_named_import(
      "import { say } from \"cowsay\";\n\nthink();\n",
      &new_import,
    )
    .unwrap();
    assert_eq!(existing_import.insert_offset, 12);
    assert_eq!(existing_import.new_text, ", think");
  }

  #[test]
  fn test_find_mergeable_named_import_multiline() {
    let new_import =
      parse_simple_named_import("import { think } from \"cowsay\";\n\n")
        .unwrap();
    let existing_import = find_mergeable_named_import(
      "import {\n  say,\n} from \"cowsay\";\n\nthink();\n",
      &new_import,
    )
    .unwrap();
    assert_eq!(existing_import.insert_offset, 16);
    assert_eq!(existing_import.new_text, "think,\n");
  }

  async fn setup(
    deno_json_content: Value,
    sources: &[(&str, &str, i32, LanguageId)],
  ) -> (TempDir, TsJsServer, Arc<StateSnapshot>) {
    let temp_dir = TempDir::new();
    let cache = LspCache::new(Some(temp_dir.url().join(".deno_dir").unwrap()));
    let mut config = Config::default();
    config
      .tree
      .inject_config_file(
        deno_config::deno_json::ConfigFile::new(
          &deno_json_content.to_string(),
          temp_dir.url().join("deno.json").unwrap(),
        )
        .unwrap(),
      )
      .await;
    let resolver =
      Arc::new(LspResolver::from_config(&config, &cache, None).await);
    let compiler_options_resolver =
      Arc::new(LspCompilerOptionsResolver::new(&config, &resolver, None));
    resolver.set_compiler_options_resolver(&compiler_options_resolver.inner);
    let linter_resolver = Arc::new(LspLinterResolver::new(
      &config,
      &compiler_options_resolver,
      &resolver,
    ));
    let mut document_modules = DocumentModules::default();
    document_modules.update_config(
      &config,
      &compiler_options_resolver,
      &resolver,
      &cache,
      &Default::default(),
    );
    for (relative_specifier, source, version, language_id) in sources {
      let specifier = temp_dir.url().join(relative_specifier).unwrap();
      document_modules.open_document(
        url_to_uri(&specifier).unwrap(),
        *version,
        *language_id,
        (*source).into(),
        None,
      );
    }
    let snapshot = Arc::new(StateSnapshot {
      project_version: 0,
      document_modules,
      config: Arc::new(config),
      compiler_options_resolver,
      linter_resolver,
      resolver,
      cache: Arc::new(cache),
      client_needs_file_uris_for_virtual_documents: false,
    });
    let performance = Arc::new(Performance::default());
    let ts_server = TsJsServer::new(performance);
    ts_server.project_changed(
      snapshot.clone(),
      &[],
      Some(
        snapshot
          .compiler_options_resolver
          .entries()
          .map(|(k, d)| (k.clone(), d.compiler_options.clone()))
          .collect(),
      ),
      None,
    );
    (temp_dir, ts_server, snapshot)
  }

  fn setup_op_state(state_snapshot: Arc<StateSnapshot>) -> OpState {
    let (_tx, rx) = mpsc::unbounded_channel();
    let state = State::new(
      state_snapshot,
      Default::default(),
      Default::default(),
      rx,
      Arc::new(AtomicBool::new(true)),
      Default::default(),
    );
    let mut op_state = OpState::new(None);
    op_state.put(state);
    op_state
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
  fn test_rewrite_first_quoted_specifier() {
    let mut text = r#"import { rollup } from "";"#.to_string();
    rewrite_first_quoted_specifier(&mut text, "$rollup");
    assert_eq!(text, r#"import { rollup } from "$rollup";"#);

    let mut text = r#"import { rollup } from "npm:rollup";"#.to_string();
    rewrite_first_quoted_specifier(&mut text, "$rollup");
    assert_eq!(text, r#"import { rollup } from "$rollup";"#);
  }

  #[tokio::test]
  async fn test_get_diagnostics() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({
        "compilerOptions": {
          "lib": [],
        },
      }),
      &[(
        "a.ts",
        r#"console.log("hello deno");"#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!([
        {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 0, "character": 7 },
          "fileName": specifier,
          "messageText": "Cannot find name 'console'. Do you need to change your target library? Try changing the \'lib\' compiler option to include 'dom'.",
          "sourceLine": "console.log(\"hello deno\");",
          "category": 1,
          "code": 2584,
        }
      ]),
    );
  }

  #[tokio::test]
  async fn test_get_diagnostics_lib() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({
        "compilerOptions": {
          "lib": ["dom"],
        },
      }),
      &[(
        "a.ts",
        r#"console.log(document.location);"#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(json!(diagnostics), json!([]));
  }

  #[tokio::test]
  async fn test_module_resolution() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({}),
      &[(
        "a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(json!(diagnostics), json!([]));
  }

  #[tokio::test]
  async fn test_bad_module_specifiers() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({}),
      &[(
        "a.ts",
        r#"
        import { A } from ".";
        "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!([
        {
          "start": {
            "line": 1,
            "character": 8
          },
          "end": {
            "line": 1,
            "character": 30
          },
          "fileName": specifier,
          "messageText": "\'A\' is declared but its value is never read.",
          "sourceLine": "        import { A } from \".\";",
          "category": 2,
          "code": 6133,
          "reportsUnnecessary": true,
        }
      ]),
    );
  }

  #[tokio::test]
  async fn test_remote_modules() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({}),
      &[(
        "a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(json!(diagnostics), json!([]));
  }

  #[tokio::test]
  async fn test_partial_modules() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({}),
      &[(
        "a.ts",
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
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!([
        {
          "start": {
            "line": 1,
            "character": 8
          },
          "end": {
            "line": 6,
            "character": 55,
          },
          "fileName": specifier.clone(),
          "messageText": "All imports in import declaration are unused.",
          "sourceLine": "        import {",
          "category": 2,
          "code": 6192,
          "reportsUnnecessary": true,
        },
        {
          "start": {
            "line": 8,
            "character": 29
          },
          "end": {
            "line": 8,
            "character": 29
          },
          "fileName": specifier,
          "messageText": "Expression expected.",
          "sourceLine": "        import * as test from",
          "category": 1,
          "code": 1109
        }
      ]),
    );
  }

  #[tokio::test]
  async fn test_no_debug_failure() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({}),
      &[(
        "a.ts",
        r#"const url = new URL("b.js", import."#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!([
        {
          "start": {
            "line": 0,
            "character": 35,
          },
          "end": {
            "line": 0,
            "character": 35
          },
          "fileName": specifier,
          "messageText": "Identifier expected.",
          "sourceLine": "const url = new URL(\"b.js\", import.",
          "category": 1,
          "code": 1003,
        }
      ]),
    );
  }

  #[tokio::test]
  async fn test_modify_sources() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({}),
      &[(
        "a.ts",
        r#"
          import * as a from "https://deno.land/x/example/a.ts";
          if (a.a === "b") {
            console.log("fail");
          }
        "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let scope = snapshot
      .config
      .tree
      .scope_for_specifier(&specifier)
      .map(|s| s.as_ref());
    let dep_specifier =
      resolve_url("https://deno.land/x/example/a.ts").unwrap();
    snapshot
      .cache
      .global()
      .set(
        &dep_specifier,
        Default::default(),
        b"export const b = \"b\";\n",
      )
      .unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(&specifier, scope, None)
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!([
        {
          "category": 1,
          "code": 2339,
          "start": {
            "line": 2,
            "character": 16,
          },
          "end": {
            "line": 2,
            "character": 17
          },
          "messageText": "Property \'a\' does not exist on type \'typeof import(\"https://deno.land/x/example/a.ts\", { with: { \"resolution-mode\": \"import\" } })\'.",
          "sourceLine": "          if (a.a === \"b\") {",
          "fileName": specifier,
        }
      ]),
    );
    let dep_document = snapshot
      .document_modules
      .documents
      .get_for_specifier(&dep_specifier, scope, &snapshot.cache)
      .unwrap();
    snapshot
      .cache
      .global()
      .set(
        &dep_specifier,
        Default::default(),
        b"export const b = \"b\";\n\nexport const a = \"b\";\n",
      )
      .unwrap();
    let snapshot = {
      Arc::new(StateSnapshot {
        project_version: snapshot.project_version + 1,
        ..snapshot.as_ref().clone()
      })
    };
    ts_server.project_changed(
      snapshot.clone(),
      &[(dep_document, ChangeKind::Modified)],
      None,
      None,
    );
    snapshot.document_modules.release(
      &dep_specifier,
      snapshot
        .config
        .tree
        .scope_for_specifier(&specifier)
        .map(|s| s.as_ref()),
      None,
    );
    let module = snapshot
      .document_modules
      .module_for_specifier(&specifier, scope, None)
      .unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot.clone(), &module, &Default::default())
      .await
      .unwrap();
    assert_eq!(json!(diagnostics), json!([]));
  }

  #[test]
  fn test_completion_entry_filter_text() {
    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "['foo']".to_string(),
      insert_text: Some("['foo']".to_string()),
      ..Default::default()
    };
    let actual = fixture.get_filter_text(None);
    assert_eq!(actual, Some(".foo".to_string()));

    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "#abc".to_string(),
      ..Default::default()
    };
    let actual = fixture.get_filter_text(None);
    assert_eq!(actual, None);

    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "#abc".to_string(),
      insert_text: Some("this.#abc".to_string()),
      ..Default::default()
    };
    let actual = fixture.get_filter_text(None);
    assert_eq!(actual, Some("abc".to_string()));
  }

  #[test]
  fn test_tsc_specifier_map_node_modules_alias_is_opt_in() {
    let map = TscSpecifierMap::new();
    let specifier = ModuleSpecifier::parse(
      "file:///project/node_modules/.deno/pkg@1.0.0/node_modules/pkg/mod.d.ts",
    )
    .unwrap();

    let denormalized = map.denormalize(&specifier, MediaType::Dts);
    assert_eq!(denormalized, specifier.as_str());

    let aliased =
      map.denormalize_with_node_modules_alias(&specifier, MediaType::Dts);
    assert_eq!(
      aliased,
      "file:///project/$node_modules/.deno/pkg@1.0.0/$node_modules/pkg/mod.d.ts",
    );
    assert_eq!(map.normalize(&aliased).unwrap(), specifier);
  }

  #[tokio::test]
  async fn test_completions() {
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
    let (temp_dir, ts_server, snapshot) =
      setup(json!({}), &[("a.ts", fixture, 1, LanguageId::TypeScript)]).await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let info = ts_server
      .get_completions(
        snapshot.clone(),
        &module,
        position,
        Some(".".to_string()),
        None,
        &Default::default(),
      )
      .await
      .unwrap()
      .unwrap();
    // 23 with stock TypeScript: `@types/node`'s `console` global exposes the
    // `Console` constructor property in addition to the 22 log methods (the
    // forked compiler suppressed it).
    assert_eq!(info.entries.len(), 23);
    let details = ts_server
      .get_completion_details(
        snapshot.clone(),
        &module,
        position,
        "log".to_string(),
        None,
        None,
        &Default::default(),
      )
      .await
      .unwrap()
      .unwrap();
    assert_eq!(
      json!(details),
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
          },
          {
            "text": " ",
            "kind": "space"
          },
          {
            "text": "(",
            "kind": "punctuation"
          },
          {
            "text": "+",
            "kind": "operator"
          },
          {
            "text": "1",
            "kind": "numericLiteral"
          },
          {
            "text": " ",
            "kind": "space"
          },
          {
            "text": "overload",
            "kind": "text"
          },
          {
            "text": ")",
            "kind": "punctuation"
          }
        ],
        "documentation": [
          {
            "text": "Outputs a message to the console",
            "kind": "text",
          },
        ],
        "tags": [
          {
            "name": "param",
            "text": [
              {
                "text": "data",
                "kind": "parameterName",
              },
              {
                "text": " ",
                "kind": "space",
              },
              {
                "text": "Values to be printed to the console",
                "kind": "text",
              },
            ],
          },
          {
            "name": "example",
            "text": [
              {
                "text": "```ts\nconsole.log('Hello', 'World', 123);\n```",
                "kind": "text",
              },
            ],
          },
        ]
      })
    );
  }

  #[tokio::test]
  async fn test_completions_fmt() {
    let fixture_a = r#"
      console.log(someLongVaria)
    "#;
    let fixture_b = r#"
      export const someLongVariable = 1
    "#;
    let line_index = LineIndex::new(fixture_a);
    let position = line_index
      .offset_tsc(lsp::Position {
        line: 1,
        character: 33,
      })
      .unwrap();
    let (temp_dir, ts_server, snapshot) = setup(
      json!({
        "fmt": {
          "semiColons": false,
          "singleQuote": true,
        },
      }),
      &[
        ("a.ts", fixture_a, 1, LanguageId::TypeScript),
        ("b.ts", fixture_b, 1, LanguageId::TypeScript),
      ],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let info = ts_server
      .get_completions(
        snapshot.clone(),
        &module,
        position,
        None,
        None,
        &Default::default(),
      )
      .await
      .unwrap()
      .unwrap();
    let entry = info
      .entries
      .iter()
      .find(|e| &e.name == "someLongVariable")
      .unwrap();
    let details = ts_server
      .get_completion_details(
        snapshot.clone(),
        &module,
        position,
        entry.name.clone(),
        entry.source.clone(),
        entry.data.clone(),
        &Default::default(),
      )
      .await
      .unwrap()
      .unwrap();
    let actions = details.code_actions.unwrap();
    let action = actions
      .iter()
      .find(|a| &a.description == r#"Add import from "./b.ts""#)
      .unwrap();
    let changes = action.changes.first().unwrap();
    let change = changes.text_changes.first().unwrap();
    assert_eq!(
      change.new_text,
      "import { someLongVariable } from './b.ts'\n"
    );
  }

  #[test]
  fn test_classification_to_semantic_tokens_multiline_tokens() {
    let line_index = LineIndex::new("  to\nken  \n");
    let classifications = Classifications {
      spans: vec![2, 6, 2057],
    };
    let semantic_tokens = classifications
      .to_semantic_tokens(&line_index, &Default::default())
      .unwrap();
    assert_eq!(
      &semantic_tokens.data,
      &[
        lsp::SemanticToken {
          delta_line: 0,
          delta_start: 2,
          length: 3,
          token_type: 7,
          token_modifiers_bitset: 9,
        },
        lsp::SemanticToken {
          delta_line: 1,
          delta_start: 0,
          length: 3,
          token_type: 7,
          token_modifiers_bitset: 9,
        },
      ]
    );
  }

  #[tokio::test]
  async fn test_get_edits_for_file_rename() {
    let (temp_dir, ts_server, snapshot) = setup(
      json!({}),
      &[
        ("a.ts", r#"import "./b.ts";"#, 1, LanguageId::TypeScript),
        ("b.ts", r#""#, 1, LanguageId::TypeScript),
      ],
    )
    .await;
    let specifier = temp_dir.url().join("b.ts").unwrap();
    let module = snapshot
      .document_modules
      .module_for_specifier(
        &specifier,
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .map(|s| s.as_ref()),
        None,
      )
      .unwrap();
    let changes = ts_server
      .get_edits_for_file_rename(
        snapshot,
        &module,
        &temp_dir.url().join("🦕.ts").unwrap(),
        &Default::default(),
      )
      .await
      .unwrap();
    assert_eq!(
      changes,
      vec![FileTextChanges {
        file_name: temp_dir.url().join("a.ts").unwrap().to_string(),
        text_changes: vec![TextChange {
          span: TextSpan {
            start: 8,
            length: 6,
          },
          new_text: "./🦕.ts".to_string(),
        }],
        is_new_file: None,
      }]
    );
  }

  #[test]
  fn include_suppress_inlay_hint_settings() {
    let mut settings = WorkspaceSettings::default();
    settings
      .typescript
      .inlay_hints
      .parameter_names
      .suppress_when_argument_matches_name = true;
    settings
      .typescript
      .inlay_hints
      .variable_types
      .suppress_when_type_matches_name = true;
    let mut config = config::Config::default();
    config.set_workspace_settings(settings, vec![]);
    let user_preferences = UserPreferences::from_config_for_specifier(
      &config,
      &ModuleSpecifier::parse("file:///foo.ts").unwrap(),
    );
    assert_eq!(
      user_preferences.include_inlay_variable_type_hints_when_type_matches_name,
      Some(false)
    );
    assert_eq!(
      user_preferences
        .include_inlay_parameter_name_hints_when_argument_matches_name,
      Some(false)
    );
  }

  #[tokio::test]
  async fn resolve_unknown_dependency() {
    let (temp_dir, _, snapshot) =
      setup(json!({}), &[("a.ts", "", 1, LanguageId::TypeScript)]).await;
    let mut state = setup_op_state(snapshot);
    let base = temp_dir.url().join("a.ts").unwrap().to_string();
    let resolved = op_resolve_inner(
      &mut state,
      ResolveArgs {
        base: &base,
        specifiers: vec![(false, "./b.ts".to_string())],
      },
    )
    .unwrap();
    assert_eq!(
      resolved,
      vec![Some((
        temp_dir.url().join("b.ts").unwrap().to_string(),
        Some(MediaType::TypeScript.as_ts_extension().to_string())
      ))]
    );
  }

  #[test]
  fn coalesce_pending_change() {
    use ChangeKind::*;
    fn change<S: AsRef<str>>(
      project_version: usize,
      scripts: impl IntoIterator<Item = (S, ChangeKind)>,
      new_compiler_options_by_key: Option<
        BTreeMap<CompilerOptionsKey, Arc<CompilerOptions>>,
      >,
    ) -> PendingChange {
      PendingChange {
        project_version,
        modified_scripts: scripts
          .into_iter()
          .map(|(s, c)| (s.as_ref().into(), c))
          .collect(),
        new_compiler_options_by_key,
        new_notebook_keys: None,
      }
    }
    let cases = [
      (
        // start
        change(1, [("file:///a.ts", Closed)], None),
        // new
        change(2, Some(("file:///b.ts", Opened)), None),
        // expected
        change(
          2,
          [("file:///a.ts", Closed), ("file:///b.ts", Opened)],
          None,
        ),
      ),
      (
        // start
        change(
          1,
          [("file:///a.ts", Closed), ("file:///b.ts", Opened)],
          None,
        ),
        // new
        change(
          2,
          // a gets closed then reopened, b gets opened then closed
          [("file:///a.ts", Opened), ("file:///b.ts", Closed)],
          None,
        ),
        // expected
        change(
          2,
          [("file:///a.ts", Opened), ("file:///b.ts", Closed)],
          None,
        ),
      ),
      (
        change(
          1,
          [("file:///a.ts", Opened), ("file:///b.ts", Modified)],
          None,
        ),
        // new
        change(
          2,
          // a gets opened then modified, b gets modified then closed
          [("file:///a.ts", Opened), ("file:///b.ts", Closed)],
          None,
        ),
        // expected
        change(
          2,
          [("file:///a.ts", Opened), ("file:///b.ts", Closed)],
          None,
        ),
      ),
    ];

    for (start, new, expected) in cases {
      let mut pending = start;
      pending.coalesce(new.project_version, new.modified_scripts, None, None);
      assert_eq!(json!(pending), json!(expected));
    }
  }
}
