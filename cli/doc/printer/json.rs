// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::doc;

#[derive(Debug)]
pub struct JSONPrinter {
  pretty_print: bool,
}

impl JSONPrinter {
  pub fn new(pretty_print: bool) -> JSONPrinter {
    JSONPrinter { pretty_print }
  }

  pub fn print(&self, doc_nodes: Vec<doc::DocNode>) {
    let docs_json = if self.pretty_print {
      serde_json::to_string_pretty(&doc_nodes).unwrap()
    } else {
      serde_json::to_string(&doc_nodes).unwrap()
    };
    println!("{}", docs_json);
  }

  #[allow(dead_code)]
  pub fn print_details(&self, node: doc::DocNode) {
    let docs_json = if self.pretty_print {
      serde_json::to_string_pretty(&node).unwrap()
    } else {
      serde_json::to_string(&node).unwrap()
    };
    println!("{}", docs_json);
  }
}
