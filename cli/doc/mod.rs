// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
pub mod class;
pub mod r#enum;
pub mod function;
pub mod interface;
pub mod module;
pub mod namespace;
mod node;
pub mod parser;
pub mod printer;
pub mod ts_type;
pub mod type_alias;
pub mod variable;

pub use node::DocNode;
pub use node::DocNodeKind;
pub use node::Location;
pub use node::ParamDef;
pub use parser::DocParser;

#[cfg(test)]
mod tests;

pub fn find_node_by_name_recursively(
  doc_nodes: Vec<DocNode>,
  name: String,
) -> Option<DocNode> {
  let mut parts = name.splitn(2, '.');
  let name = parts.next();
  let leftover = parts.next();
  name?;
  let node = find_node_by_name(doc_nodes, name.unwrap().to_string());
  match node {
    Some(node) => match node.kind {
      DocNodeKind::Namespace => {
        if let Some(leftover) = leftover {
          find_node_by_name_recursively(
            node.namespace_def.unwrap().elements,
            leftover.to_string(),
          )
        } else {
          Some(node)
        }
      }
      _ => {
        if leftover.is_none() {
          Some(node)
        } else {
          None
        }
      }
    },
    _ => None,
  }
}

fn find_node_by_name(doc_nodes: Vec<DocNode>, name: String) -> Option<DocNode> {
  let node = doc_nodes.iter().find(|node| node.name == name);
  match node {
    Some(node) => Some(node.clone()),
    None => None,
  }
}
