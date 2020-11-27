// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use lsp_types::request::Request;
use lsp_types::TextDocumentIdentifier;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualTextDocumentParams {
  pub text_document: TextDocumentIdentifier,
}

pub enum VirtualTextDocument {}

impl Request for VirtualTextDocument {
  type Params = VirtualTextDocumentParams;
  type Result = String;
  const METHOD: &'static str = "deno/virtualTextDocument";
}
