// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
pub mod class;
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
pub use node::Location;
pub use node::ParamDef;
pub use node::ParamKind;
pub use parser::DocParser;

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
  let mut filtered: Vec<DocNode> = vec![];
  for node in doc_nodes {
    if node.name == name {
      filtered.push(node);
    }
  }

  let mut found: Vec<DocNode> = vec![];
  match leftover {
    Some(leftover) => {
      for node in filtered {
        let children = match node.kind {
          DocNodeKind::Namespace => {
            let namespace_def = node.namespace_def.unwrap();
            find_nodes_by_name_recursively(
              namespace_def.elements,
              leftover.to_string(),
            )
          }
          // TODO(#4516) handle class, interface etc...
          _ => vec![],
        };
        found.extend(children);
      }
      found
    }
    None => filtered,
  }
}
