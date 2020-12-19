// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

///!
///! Extensions to the language service protocol that are specific to Deno.
///!
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use lsp_types::request::Request;
use lsp_types::TextDocumentIdentifier;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualTextDocumentParams {
  pub text_document: TextDocumentIdentifier,
}

/// Request a _virtual_ text document from the server. Used for example to
/// provide a status document of the language server which can be viewed in the
/// IDE.
pub enum VirtualTextDocument {}

impl Request for VirtualTextDocument {
  type Params = VirtualTextDocumentParams;
  type Result = String;
  const METHOD: &'static str = "deno/virtualTextDocument";
}
