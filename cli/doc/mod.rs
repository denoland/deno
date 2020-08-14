// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
pub mod class;
mod display;
pub mod r#enum;
pub mod function;
pub mod interface;
pub mod module;
pub mod namespace;
mod node;
pub mod params;
pub mod parser;
pub mod printer;
pub mod ts_type;
pub mod ts_type_param;
pub mod type_alias;
pub mod variable;

pub use node::DocNode;
pub use node::DocNodeKind;
pub use node::ImportDef;
pub use node::Location;
pub use params::ParamDef;
pub use parser::DocParser;
pub use printer::DocPrinter;

#[cfg(test)]
mod tests;

pub fn find_nodes_by_name_recursively(
  doc_nodes: Vec<DocNode>,
  name: String,
) -> Vec<DocNode> {
  let mut parts = name.splitn(2, '.');
  let name = parts.next();
  let leftover = parts.next();
  if name.is_none() {
    return doc_nodes;
  }

  let name = name.unwrap();
  let doc_nodes = find_nodes_by_name(doc_nodes, name.to_string());

  let mut found: Vec<DocNode> = vec![];
  match leftover {
    Some(leftover) => {
      for node in doc_nodes {
        let children = find_children_by_name(node, leftover.to_string());
        found.extend(children);
      }
      found
    }
    None => doc_nodes,
  }
}

fn find_nodes_by_name(doc_nodes: Vec<DocNode>, name: String) -> Vec<DocNode> {
  let mut found: Vec<DocNode> = vec![];
  for node in doc_nodes {
    if node.name == name {
      found.push(node);
    }
  }
  found
}

fn find_children_by_name(node: DocNode, name: String) -> Vec<DocNode> {
  match node.kind {
    DocNodeKind::Namespace => {
      let namespace_def = node.namespace_def.unwrap();
      find_nodes_by_name_recursively(namespace_def.elements, name)
    }
    // TODO(#4516) handle class, interface etc...
    _ => vec![],
  }
}
