// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// TODO(ry) This module builds up output by appending to a string. Instead it
// should either use a formatting trait
// https://doc.rust-lang.org/std/fmt/index.html#formatting-traits
// Or perhaps implement a Serializer for serde
// https://docs.serde.rs/serde/ser/trait.Serializer.html

// TODO(ry) The methods in this module take ownership of the DocNodes, this is
// unnecessary and can result in unnecessary copying. Instead they should take
// references.

use crate::colors;
use crate::doc;
use crate::doc::ts_type::TsTypeDefKind;
use crate::doc::DocNodeKind;
use crate::swc_ecma_ast;

pub fn format(doc_nodes: Vec<doc::DocNode>) -> String {
  format_(doc_nodes, 0)
}

pub fn format_details(node: doc::DocNode) -> String {
  let mut details = String::new();

  details.push_str(&format!(
    "{}",
    colors::gray(format!(
      "Defined in {}:{}:{} \n\n",
      node.location.filename, node.location.line, node.location.col
    ))
  ));

  details.push_str(&format_signature(&node, 0));

  let js_doc = node.js_doc.clone();
  if let Some(js_doc) = js_doc {
    details.push_str(&format_jsdoc(js_doc, 1));
  }
  details.push_str("\n");

  let maybe_extra = match node.kind {
    DocNodeKind::Class => Some(format_class_details(node)),
    DocNodeKind::Enum => Some(format_enum_details(node)),
    DocNodeKind::Namespace => Some(format_namespace_details(node)),
    _ => None,
  };

  if let Some(extra) = maybe_extra {
    details.push_str(&extra);
  }

  details
}

fn kind_order(kind: &doc::DocNodeKind) -> i64 {
  match kind {
    DocNodeKind::Function => 0,
    DocNodeKind::Variable => 1,
    DocNodeKind::Class => 2,
    DocNodeKind::Enum => 3,
    DocNodeKind::Interface => 4,
    DocNodeKind::TypeAlias => 5,
    DocNodeKind::Namespace => 6,
  }
}

fn format_signature(node: &doc::DocNode, indent: i64) -> String {
  match node.kind {
    DocNodeKind::Function => format_function_signature(&node, indent),
    DocNodeKind::Variable => format_variable_signature(&node, indent),
    DocNodeKind::Class => format_class_signature(&node, indent),
    DocNodeKind::Enum => format_enum_signature(&node, indent),
    DocNodeKind::Interface => format_interface_signature(&node, indent),
    DocNodeKind::TypeAlias => format_type_alias_signature(&node, indent),
    DocNodeKind::Namespace => format_namespace_signature(&node, indent),
  }
}

fn format_(doc_nodes: Vec<doc::DocNode>, indent: i64) -> String {
  let mut sorted = doc_nodes;
  sorted.sort_unstable_by(|a, b| {
    let kind_cmp = kind_order(&a.kind).cmp(&kind_order(&b.kind));
    if kind_cmp == core::cmp::Ordering::Equal {
      a.name.cmp(&b.name)
    } else {
      kind_cmp
    }
  });

  let mut output = String::new();

  for node in sorted {
    output.push_str(&format_signature(&node, indent));
    if let Some(js_doc) = node.js_doc {
      output.push_str(&format_jsdoc(js_doc, indent));
    }
    output.push_str("\n");
    if DocNodeKind::Namespace == node.kind {
      output.push_str(&format_(
        node.namespace_def.as_ref().unwrap().elements.clone(),
        indent + 1,
      ));
      output.push_str("\n");
    };
  }

  output
}

fn render_params(params: Vec<doc::ParamDef>) -> String {
  let mut rendered = String::from("");
  if !params.is_empty() {
    for param in params {
      rendered += param.name.as_str();
      if param.optional {
        rendered += "?";
      }
      if let Some(ts_type) = param.ts_type {
        rendered += ": ";
        rendered += render_ts_type(ts_type).as_str();
      }
      rendered += ", ";
    }
    rendered.truncate(rendered.len() - 2);
  }
  rendered
}

fn render_ts_type(ts_type: doc::ts_type::TsTypeDef) -> String {
  if ts_type.kind.is_none() {
    return "<UNIMPLEMENTED>".to_string();
  }
  let kind = ts_type.kind.unwrap();
  match kind {
    TsTypeDefKind::Array => {
      format!("{}[]", render_ts_type(*ts_type.array.unwrap()))
    }
    TsTypeDefKind::Conditional => {
      let conditional = ts_type.conditional_type.unwrap();
      format!(
        "{} extends {} ? {} : {}",
        render_ts_type(*conditional.check_type),
        render_ts_type(*conditional.extends_type),
        render_ts_type(*conditional.true_type),
        render_ts_type(*conditional.false_type)
      )
    }
    TsTypeDefKind::FnOrConstructor => {
      let fn_or_constructor = ts_type.fn_or_constructor.unwrap();
      format!(
        "{}({}) => {}",
        if fn_or_constructor.constructor {
          "new "
        } else {
          ""
        },
        render_params(fn_or_constructor.params),
        render_ts_type(fn_or_constructor.ts_type),
      )
    }
    TsTypeDefKind::IndexedAccess => {
      let indexed_access = ts_type.indexed_access.unwrap();
      format!(
        "{}[{}]",
        render_ts_type(*indexed_access.obj_type),
        render_ts_type(*indexed_access.index_type)
      )
    }
    TsTypeDefKind::Intersection => {
      let intersection = ts_type.intersection.unwrap();
      let mut output = "".to_string();
      if !intersection.is_empty() {
        for ts_type in intersection {
          output += render_ts_type(ts_type).as_str();
          output += " & "
        }
        output.truncate(output.len() - 3);
      }
      output
    }
    TsTypeDefKind::Keyword => ts_type.keyword.unwrap(),
    TsTypeDefKind::Literal => {
      let literal = ts_type.literal.unwrap();
      match literal.kind {
        doc::ts_type::LiteralDefKind::Boolean => {
          format!("{}", literal.boolean.unwrap())
        }
        doc::ts_type::LiteralDefKind::String => {
          "\"".to_string() + literal.string.unwrap().as_str() + "\""
        }
        doc::ts_type::LiteralDefKind::Number => {
          format!("{}", literal.number.unwrap())
        }
      }
    }
    TsTypeDefKind::Optional => {
      format!("{}?", render_ts_type(*ts_type.optional.unwrap()))
    }
    TsTypeDefKind::Parenthesized => {
      format!("({})", render_ts_type(*ts_type.parenthesized.unwrap()))
    }
    TsTypeDefKind::Rest => {
      format!("...{}", render_ts_type(*ts_type.rest.unwrap()))
    }
    TsTypeDefKind::This => "this".to_string(),
    TsTypeDefKind::Tuple => {
      let tuple = ts_type.tuple.unwrap();
      let mut output = "".to_string();
      if !tuple.is_empty() {
        for ts_type in tuple {
          output += render_ts_type(ts_type).as_str();
          output += ", "
        }
        output.truncate(output.len() - 2);
      }
      output
    }
    TsTypeDefKind::TypeLiteral => {
      let mut output = "".to_string();
      let type_literal = ts_type.type_literal.unwrap();
      for node in type_literal.call_signatures {
        output += format!(
          "({}){}, ",
          render_params(node.params),
          if let Some(ts_type) = node.ts_type {
            format!(": {}", render_ts_type(ts_type))
          } else {
            "".to_string()
          }
        )
        .as_str()
      }
      for node in type_literal.methods {
        output += format!(
          "{}({}){}, ",
          node.name,
          render_params(node.params),
          if let Some(return_type) = node.return_type {
            format!(": {}", render_ts_type(return_type))
          } else {
            "".to_string()
          }
        )
        .as_str()
      }
      for node in type_literal.properties {
        output += format!(
          "{}{}, ",
          node.name,
          if let Some(ts_type) = node.ts_type {
            format!(": {}", render_ts_type(ts_type))
          } else {
            "".to_string()
          }
        )
        .as_str()
      }
      if !output.is_empty() {
        output.truncate(output.len() - 2);
      }
      "{ ".to_string() + output.as_str() + " }"
    }
    TsTypeDefKind::TypeOperator => {
      let operator = ts_type.type_operator.unwrap();
      format!("{} {}", operator.operator, render_ts_type(operator.ts_type))
    }
    TsTypeDefKind::TypeQuery => {
      format!("typeof {}", ts_type.type_query.unwrap())
    }
    TsTypeDefKind::TypeRef => {
      let type_ref = ts_type.type_ref.unwrap();
      let mut final_output = type_ref.type_name;
      if let Some(type_params) = type_ref.type_params {
        let mut output = "".to_string();
        if !type_params.is_empty() {
          for ts_type in type_params {
            output += render_ts_type(ts_type).as_str();
            output += ", "
          }
          output.truncate(output.len() - 2);
        }
        final_output += format!("<{}>", output).as_str();
      }
      final_output
    }
    TsTypeDefKind::Union => {
      let union = ts_type.union.unwrap();
      let mut output = "".to_string();
      if !union.is_empty() {
        for ts_type in union {
          output += render_ts_type(ts_type).as_str();
          output += " | "
        }
        output.truncate(output.len() - 3);
      }
      output
    }
  }
}

fn add_indent(string: String, indent: i64) -> String {
  let mut indent_str = String::new();
  for _ in 0..(indent * 2) {
    indent_str += " ";
  }
  indent_str += string.as_str();
  indent_str
}

// TODO: this should use some sort of markdown to console parser.
fn format_jsdoc(jsdoc: String, indent: i64) -> String {
  let lines = jsdoc.split("\n\n").map(|line| line.replace("\n", " "));

  let mut js_doc = String::new();

  for line in lines {
    js_doc.push_str(&add_indent(format!("{}\n", line), indent + 1));
  }

  format!("{}", colors::gray(js_doc))
}

fn format_class_details(node: doc::DocNode) -> String {
  let mut details = String::new();

  let class_def = node.class_def.unwrap();
  for node in class_def.constructors {
    details.push_str(&add_indent(
      format!(
        "{} {}({})\n",
        colors::magenta("constructor".to_string()),
        colors::bold(node.name),
        render_params(node.params),
      ),
      1,
    ));
  }
  for node in class_def.properties.iter().filter(|node| {
    node
      .accessibility
      .unwrap_or(swc_ecma_ast::Accessibility::Public)
      != swc_ecma_ast::Accessibility::Private
  }) {
    details.push_str(&add_indent(
      format!(
        "{}{}{}{}\n",
        colors::magenta(
          match node
            .accessibility
            .unwrap_or(swc_ecma_ast::Accessibility::Public)
          {
            swc_ecma_ast::Accessibility::Protected => "protected ".to_string(),
            _ => "".to_string(),
          }
        ),
        colors::bold(node.name.clone()),
        if node.optional {
          "?".to_string()
        } else {
          "".to_string()
        },
        if let Some(ts_type) = node.ts_type.clone() {
          format!(": {}", render_ts_type(ts_type))
        } else {
          "".to_string()
        }
      ),
      1,
    ));
  }
  for node in class_def.methods.iter().filter(|node| {
    node
      .accessibility
      .unwrap_or(swc_ecma_ast::Accessibility::Public)
      != swc_ecma_ast::Accessibility::Private
  }) {
    let function_def = node.function_def.clone();
    details.push_str(&add_indent(
      format!(
        "{}{}{}{}({}){}\n",
        colors::magenta(
          match node
            .accessibility
            .unwrap_or(swc_ecma_ast::Accessibility::Public)
          {
            swc_ecma_ast::Accessibility::Protected => "protected ".to_string(),
            _ => "".to_string(),
          }
        ),
        colors::magenta(match node.kind {
          swc_ecma_ast::MethodKind::Getter => "get ".to_string(),
          swc_ecma_ast::MethodKind::Setter => "set ".to_string(),
          _ => "".to_string(),
        }),
        colors::bold(node.name.clone()),
        if node.optional {
          "?".to_string()
        } else {
          "".to_string()
        },
        render_params(function_def.params),
        if let Some(return_type) = function_def.return_type {
          format!(": {}", render_ts_type(return_type))
        } else {
          "".to_string()
        }
      ),
      1,
    ));
  }
  details.push_str("\n");
  details
}

fn format_enum_details(node: doc::DocNode) -> String {
  let mut details = String::new();
  let enum_def = node.enum_def.unwrap();
  for member in enum_def.members {
    details
      .push_str(&add_indent(format!("{}\n", colors::bold(member.name)), 1));
  }
  details.push_str("\n");
  details
}

fn format_namespace_details(node: doc::DocNode) -> String {
  let mut ns = String::new();

  let elements = node.namespace_def.unwrap().elements;
  for node in elements {
    ns.push_str(&format_signature(&node, 1));
  }
  ns.push_str("\n");
  ns
}

fn format_function_signature(node: &doc::DocNode, indent: i64) -> String {
  let function_def = node.function_def.clone().unwrap();
  add_indent(
    format!(
      "{} {}({}){}\n",
      colors::magenta("function".to_string()),
      colors::bold(node.name.clone()),
      render_params(function_def.params),
      if let Some(return_type) = function_def.return_type {
        format!(": {}", render_ts_type(return_type).as_str())
      } else {
        "".to_string()
      }
    ),
    indent,
  )
}

fn format_class_signature(node: &doc::DocNode, indent: i64) -> String {
  let class_def = node.class_def.clone().unwrap();
  let extends_suffix = if let Some(extends) = class_def.extends {
    format!(
      " {} {}",
      colors::magenta("extends".to_string()),
      colors::bold(extends)
    )
  } else {
    String::from("")
  };

  let implements = &class_def.implements;
  let implements_suffix = if !implements.is_empty() {
    format!(
      " {} {}",
      colors::magenta("implements".to_string()),
      colors::bold(implements.join(", "))
    )
  } else {
    String::from("")
  };

  add_indent(
    format!(
      "{} {}{}{}\n",
      colors::magenta("class".to_string()),
      colors::bold(node.name.clone()),
      extends_suffix,
      implements_suffix,
    ),
    indent,
  )
}

fn format_variable_signature(node: &doc::DocNode, indent: i64) -> String {
  let variable_def = node.variable_def.clone().unwrap();
  add_indent(
    format!(
      "{} {}{}\n",
      colors::magenta(match variable_def.kind {
        swc_ecma_ast::VarDeclKind::Const => "const".to_string(),
        swc_ecma_ast::VarDeclKind::Let => "let".to_string(),
        swc_ecma_ast::VarDeclKind::Var => "var".to_string(),
      }),
      colors::bold(node.name.clone()),
      if let Some(ts_type) = variable_def.ts_type {
        format!(": {}", render_ts_type(ts_type))
      } else {
        "".to_string()
      }
    ),
    indent,
  )
}

fn format_enum_signature(node: &doc::DocNode, indent: i64) -> String {
  add_indent(
    format!(
      "{} {}\n",
      colors::magenta("enum".to_string()),
      colors::bold(node.name.clone())
    ),
    indent,
  )
}

fn format_interface_signature(node: &doc::DocNode, indent: i64) -> String {
  let interface_def = node.interface_def.clone().unwrap();
  let extends = &interface_def.extends;
  let extends_suffix = if !extends.is_empty() {
    format!(
      " {} {}",
      colors::magenta("extends".to_string()),
      colors::bold(extends.join(", "))
    )
  } else {
    String::from("")
  };
  add_indent(
    format!(
      "{} {}{}\n",
      colors::magenta("interface".to_string()),
      colors::bold(node.name.clone()),
      extends_suffix
    ),
    indent,
  )
}

fn format_type_alias_signature(node: &doc::DocNode, indent: i64) -> String {
  add_indent(
    format!(
      "{} {}\n",
      colors::magenta("type".to_string()),
      colors::bold(node.name.clone())
    ),
    indent,
  )
}

fn format_namespace_signature(node: &doc::DocNode, indent: i64) -> String {
  add_indent(
    format!(
      "{} {}\n",
      colors::magenta("namespace".to_string()),
      colors::bold(node.name.clone())
    ),
    indent,
  )
}
