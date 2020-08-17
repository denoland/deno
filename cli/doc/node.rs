// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum DocNodeKind {
  Function,
  Variable,
  Class,
  Enum,
  Interface,
  TypeAlias,
  Namespace,
  Import,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Location {
  pub filename: String,
  pub line: usize,
  pub col: usize,
}

impl Into<Location> for swc_common::Loc {
  fn into(self) -> Location {
    use swc_common::FileName::*;

    let filename = match &self.file.name {
      Real(path_buf) => path_buf.to_string_lossy().to_string(),
      Custom(str_) => str_.to_string(),
      _ => panic!("invalid filename"),
    };

    Location {
      filename,
      line: self.line,
      col: self.col_display,
    }
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ReexportKind {
  /// export * from "./path/to/module.js";
  All,
  /// export * as someNamespace from "./path/to/module.js";
  Namespace(String),
  /// export default from "./path/to/module.js";
  Default,
  /// (identifier, optional alias)
  /// export { foo } from "./path/to/module.js";
  /// export { foo as bar } from "./path/to/module.js";
  Named(String, Option<String>),
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Reexport {
  pub kind: ReexportKind,
  pub src: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModuleDoc {
  pub definitions: Vec<DocNode>,
  pub reexports: Vec<Reexport>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportDef {
  pub src: String,
  pub imported: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocNode {
  pub kind: DocNodeKind,
  pub name: String,
  pub location: Location,
  pub js_doc: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub function_def: Option<super::function::FunctionDef>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub variable_def: Option<super::variable::VariableDef>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub enum_def: Option<super::r#enum::EnumDef>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub class_def: Option<super::class::ClassDef>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub type_alias_def: Option<super::type_alias::TypeAliasDef>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub namespace_def: Option<super::namespace::NamespaceDef>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub interface_def: Option<super::interface::InterfaceDef>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub import_def: Option<ImportDef>,
}

impl DocNode {
  pub fn function(
    name: String,
    location: Location,
    js_doc: Option<String>,
    fn_def: super::function::FunctionDef,
  ) -> Self {
    Self {
      kind: DocNodeKind::Function,
      name,
      location,
      js_doc,
      function_def: Some(fn_def),
      variable_def: None,
      enum_def: None,
      class_def: None,
      type_alias_def: None,
      namespace_def: None,
      interface_def: None,
      import_def: None,
    }
  }

  pub fn variable(
    name: String,
    location: Location,
    js_doc: Option<String>,
    var_def: super::variable::VariableDef,
  ) -> Self {
    Self {
      kind: DocNodeKind::Variable,
      name,
      location,
      js_doc,
      function_def: None,
      variable_def: Some(var_def),
      enum_def: None,
      class_def: None,
      type_alias_def: None,
      namespace_def: None,
      interface_def: None,
      import_def: None,
    }
  }

  pub fn r#enum(
    name: String,
    location: Location,
    js_doc: Option<String>,
    enum_def: super::r#enum::EnumDef,
  ) -> Self {
    Self {
      kind: DocNodeKind::Enum,
      name,
      location,
      js_doc,
      function_def: None,
      variable_def: None,
      enum_def: Some(enum_def),
      class_def: None,
      type_alias_def: None,
      namespace_def: None,
      interface_def: None,
      import_def: None,
    }
  }

  pub fn class(
    name: String,
    location: Location,
    js_doc: Option<String>,
    class_def: super::class::ClassDef,
  ) -> Self {
    Self {
      kind: DocNodeKind::Class,
      name,
      location,
      js_doc,
      function_def: None,
      variable_def: None,
      enum_def: None,
      class_def: Some(class_def),
      type_alias_def: None,
      namespace_def: None,
      interface_def: None,
      import_def: None,
    }
  }

  pub fn type_alias(
    name: String,
    location: Location,
    js_doc: Option<String>,
    type_alias_def: super::type_alias::TypeAliasDef,
  ) -> Self {
    Self {
      kind: DocNodeKind::TypeAlias,
      name,
      location,
      js_doc,
      function_def: None,
      variable_def: None,
      enum_def: None,
      class_def: None,
      type_alias_def: Some(type_alias_def),
      namespace_def: None,
      interface_def: None,
      import_def: None,
    }
  }

  pub fn namespace(
    name: String,
    location: Location,
    js_doc: Option<String>,
    namespace_def: super::namespace::NamespaceDef,
  ) -> Self {
    Self {
      kind: DocNodeKind::Namespace,
      name,
      location,
      js_doc,
      function_def: None,
      variable_def: None,
      enum_def: None,
      class_def: None,
      type_alias_def: None,
      namespace_def: Some(namespace_def),
      interface_def: None,
      import_def: None,
    }
  }

  pub fn interface(
    name: String,
    location: Location,
    js_doc: Option<String>,
    interface_def: super::interface::InterfaceDef,
  ) -> Self {
    Self {
      kind: DocNodeKind::Interface,
      name,
      location,
      js_doc,
      function_def: None,
      variable_def: None,
      enum_def: None,
      class_def: None,
      type_alias_def: None,
      namespace_def: None,
      interface_def: Some(interface_def),
      import_def: None,
    }
  }

  pub fn import(
    name: String,
    location: Location,
    js_doc: Option<String>,
    import_def: ImportDef,
  ) -> Self {
    Self {
      kind: DocNodeKind::Import,
      name,
      location,
      js_doc,
      function_def: None,
      variable_def: None,
      enum_def: None,
      class_def: None,
      type_alias_def: None,
      namespace_def: None,
      interface_def: None,
      import_def: Some(import_def),
    }
  }
}
