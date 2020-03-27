// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::DocNode;

mod json;
mod terminal;
pub use json::JSONPrinter;
pub use terminal::TerminalPrinter;

pub trait Printer {
  fn print(&self, doc_nodes: Vec<DocNode>);
  fn print_details(&self, node: DocNode);
}
