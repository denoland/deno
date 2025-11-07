// Copyright 2018-2025 the Deno authors. MIT license.

use std::marker::PhantomData;

use indexmap::IndexMap;

#[derive(serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Position {
  pub line: u64,
  pub character: u64,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
  pub file_name: String,
  pub start: Position,
  pub end: Position,
  pub start_pos: u32,
  pub end_pos: u32,
  pub code: u32,
  pub category: String,
  pub message: String,
  pub message_chain: Vec<Diagnostic>,
  pub related_information: Vec<Diagnostic>,
  pub reports_unnecessary: bool,
  pub reports_deprecated: bool,
  pub skipped_on_no_emit: bool,
  pub source_line: String,
}

pub type DiagnosticId = u32;

#[derive(
  serde_repr::Deserialize_repr, serde_repr::Serialize_repr, Debug, Clone, Copy,
)]
#[repr(u32)]
pub enum ResolutionMode {
  None = 0,
  CommonJS = 1,
  ESM = 99,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResolveModuleNamePayload {
  pub module_name: String,
  pub containing_file: String,
  pub resolution_mode: ResolutionMode,
  pub import_attribute_type: Option<String>,
  // redirected_reference: Handle<Project>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResolveTypeReferenceDirectivePayload {
  pub type_reference_directive_name: String,
  pub containing_file: String,
  pub resolution_mode: ResolutionMode,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(from = "String", into = "String")]
pub struct Handle<T> {
  pub id: String,
  #[serde(skip)]
  _phantom: PhantomData<T>,
}

impl<T> std::fmt::Debug for Handle<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Handle").field(&self.id).finish()
  }
}

impl<T> From<Handle<T>> for String {
  fn from(value: Handle<T>) -> Self {
    value.id
  }
}

impl<T> Clone for Handle<T> {
  fn clone(&self) -> Self {
    Self {
      id: self.id.clone(),
      _phantom: PhantomData,
    }
  }
}

impl<T> From<String> for Handle<T> {
  fn from(id: String) -> Self {
    Self {
      id,
      _phantom: PhantomData,
    }
  }
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
  pub id: Handle<Self>,
  pub config_file_name: String,
  pub root_files: Vec<String>,
  pub compiler_options: IndexMap<String, serde_json::Value>,
}

#[derive(
  Debug, Clone, serde_repr::Deserialize_repr, serde_repr::Serialize_repr,
)]
#[repr(u32)]
pub enum ModuleKind {
  None = 0,
  CommonJS = 1,
  AMD = 2,
  UMD = 3,
  System = 4,
  ES2015 = 5,
  ES2020 = 6,
  ES2022 = 7,
  ESNext = 99,
  Node16 = 100,
  Node18 = 101,
  NodeNext = 199,
  Preserve = 200,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetImpliedNodeFormatForFilePayload {
  pub file_name: String,
  pub package_json_type: String,
}
