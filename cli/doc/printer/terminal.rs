// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::doc;
use crate::doc::ts_type::TsTypeDefKind;
use crate::doc::DocNodeKind;

#[derive(Debug)]
pub struct TerminalPrinter {}

impl TerminalPrinter {
  pub fn new() -> TerminalPrinter {
    TerminalPrinter {}
  }

  pub fn print(&self, doc_nodes: Vec<doc::DocNode>) {
    self.print_(doc_nodes, 0);
  }

  pub fn print_details(&self, node: doc::DocNode) {
    println!(
      "Defined in {}:{}:{}.\n",
      node.location.filename, node.location.line, node.location.col
    );

    self.print_signature(&node, 0);

    let js_doc = node.js_doc.clone();
    if let Some(js_doc) = js_doc {
      self.print_jsdoc(js_doc, false, 1);
    }
    println!();

    match node.kind {
      DocNodeKind::Class => self.print_class_details(node),
      DocNodeKind::Namespace => self.print_namespace_details(node),
      _ => {}
    };
  }

  fn kind_order(&self, kind: &doc::DocNodeKind) -> i64 {
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

  fn print_signature(&self, node: &doc::DocNode, indent: i64) {
    match node.kind {
      DocNodeKind::Function => self.print_function_signature(&node, indent),
      DocNodeKind::Variable => self.print_variable_signature(&node, indent),
      DocNodeKind::Class => self.print_class_signature(&node, indent),
      DocNodeKind::Enum => self.print_enum_signature(&node, indent),
      DocNodeKind::Interface => self.print_interface_signature(&node, indent),
      DocNodeKind::TypeAlias => self.print_type_alias_signature(&node, indent),
      DocNodeKind::Namespace => self.print_namespace_signature(&node, indent),
    };
  }

  fn print_(&self, doc_nodes: Vec<doc::DocNode>, indent: i64) {
    let mut sorted = doc_nodes;
    sorted.sort_unstable_by(|a, b| {
      let kind_cmp = self.kind_order(&a.kind).cmp(&self.kind_order(&b.kind));
      if kind_cmp == core::cmp::Ordering::Equal {
        a.name.cmp(&b.name)
      } else {
        kind_cmp
      }
    });

    for node in sorted {
      self.print_signature(&node, indent);
      if node.js_doc.is_some() {
        self.print_jsdoc(node.js_doc.unwrap(), true, indent);
      }
      println!();
      if DocNodeKind::Namespace == node.kind {
        self.print_(node.namespace_def.unwrap().elements, indent + 1);
        println!();
      };
    }
  }

  fn render_params(&self, params: Vec<doc::ParamDef>) -> String {
    let mut rendered = String::from("");
    if !params.is_empty() {
      for param in params {
        rendered += param.name.as_str();
        if param.ts_type.is_some() {
          rendered += ": ";
          rendered += self.render_ts_type(param.ts_type.unwrap()).as_str();
        }
        rendered += ", ";
      }
      rendered.truncate(rendered.len() - 2);
    }
    rendered
  }

  fn render_ts_type(&self, ts_type: doc::ts_type::TsTypeDef) -> String {
    let kind = ts_type.kind.unwrap();
    match kind {
      TsTypeDefKind::Array => {
        format!("{}[]", self.render_ts_type(*ts_type.array.unwrap()))
      }
      TsTypeDefKind::Conditional => {
        let conditional = ts_type.conditional_type.unwrap();
        format!(
          "{} extends {} ? {} : {}",
          self.render_ts_type(*conditional.check_type),
          self.render_ts_type(*conditional.extends_type),
          self.render_ts_type(*conditional.true_type),
          self.render_ts_type(*conditional.false_type)
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
          self.render_params(fn_or_constructor.params),
          self.render_ts_type(fn_or_constructor.ts_type),
        )
      }
      TsTypeDefKind::IndexedAccess => {
        let indexed_access = ts_type.indexed_access.unwrap();
        format!(
          "{}[{}]",
          self.render_ts_type(*indexed_access.obj_type),
          self.render_ts_type(*indexed_access.index_type)
        )
      }
      TsTypeDefKind::Intersection => {
        let intersection = ts_type.intersection.unwrap();
        let mut output = "".to_string();
        if !intersection.is_empty() {
          for ts_type in intersection {
            output += self.render_ts_type(ts_type).as_str();
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
      TsTypeDefKind::Optional => "_optional_".to_string(),
      TsTypeDefKind::Parenthesized => {
        format!("({})", self.render_ts_type(*ts_type.parenthesized.unwrap()))
      }
      TsTypeDefKind::Rest => {
        format!("...{}", self.render_ts_type(*ts_type.rest.unwrap()))
      }
      TsTypeDefKind::This => "this".to_string(),
      TsTypeDefKind::Tuple => {
        let tuple = ts_type.tuple.unwrap();
        let mut output = "".to_string();
        if !tuple.is_empty() {
          for ts_type in tuple {
            output += self.render_ts_type(ts_type).as_str();
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
            "({}): {}, ",
            self.render_params(node.params),
            self.render_ts_type(node.ts_type.unwrap())
          )
          .as_str()
        }
        for node in type_literal.methods {
          output += format!(
            "{}({}): {}, ",
            node.name,
            self.render_params(node.params),
            self.render_ts_type(node.return_type.unwrap())
          )
          .as_str()
        }
        for node in type_literal.properties {
          output += format!(
            "{}: {}, ",
            node.name,
            self.render_ts_type(node.ts_type.unwrap())
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
        format!(
          "{} {}",
          operator.operator,
          self.render_ts_type(operator.ts_type)
        )
      }
      TsTypeDefKind::TypeQuery => {
        format!("typeof {}", ts_type.type_query.unwrap())
      }
      TsTypeDefKind::TypeRef => {
        let type_ref = ts_type.type_ref.unwrap();
        let mut final_output = type_ref.type_name;
        if type_ref.type_params.is_some() {
          let mut output = "".to_string();
          let type_params = type_ref.type_params.unwrap();
          if !type_params.is_empty() {
            for ts_type in type_params {
              output += self.render_ts_type(ts_type).as_str();
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
            output += self.render_ts_type(ts_type).as_str();
            output += " | "
          }
          output.truncate(output.len() - 3);
        }
        output
      }
    }
  }

  fn print_indent(&self, indent: i64) {
    for _ in 0..indent {
      print!("  ")
    }
  }

  // TODO: this should use some sort of markdown to console parser.
  fn print_jsdoc(&self, jsdoc: String, truncated: bool, indent: i64) {
    let mut lines = jsdoc.split("\n\n").map(|line| line.replace("\n", " "));
    if truncated {
      let first_line = lines.next().unwrap_or_else(|| "".to_string());
      self.print_indent(indent + 1);
      println!("{}", first_line)
    } else {
      for line in lines {
        self.print_indent(indent + 1);
        println!("{}", line)
      }
    }
  }

  fn print_class_details(&self, node: doc::DocNode) {
    let class_def = node.class_def.unwrap();
    for node in class_def.constructors {
      println!(
        "constructor {}({})",
        node.name,
        self.render_params(node.params),
      );
    }
    for node in class_def.properties.iter().filter(|node| {
      node
        .accessibility
        .unwrap_or(swc_ecma_ast::Accessibility::Public)
        != swc_ecma_ast::Accessibility::Private
    }) {
      println!(
        "{} {}: {}",
        match node
          .accessibility
          .unwrap_or(swc_ecma_ast::Accessibility::Public)
        {
          swc_ecma_ast::Accessibility::Protected => "protected".to_string(),
          swc_ecma_ast::Accessibility::Public => "public".to_string(),
          _ => "".to_string(),
        },
        node.name,
        self.render_ts_type(node.ts_type.clone().unwrap())
      );
    }
    for node in class_def.methods.iter().filter(|node| {
      node
        .accessibility
        .unwrap_or(swc_ecma_ast::Accessibility::Public)
        != swc_ecma_ast::Accessibility::Private
    }) {
      let function_def = node.function_def.clone();
      println!(
        "{} {}{}({}): {}",
        match node
          .accessibility
          .unwrap_or(swc_ecma_ast::Accessibility::Public)
        {
          swc_ecma_ast::Accessibility::Protected => "protected".to_string(),
          swc_ecma_ast::Accessibility::Public => "public".to_string(),
          _ => "".to_string(),
        },
        match node.kind {
          swc_ecma_ast::MethodKind::Getter => "get ".to_string(),
          swc_ecma_ast::MethodKind::Setter => "set ".to_string(),
          _ => "".to_string(),
        },
        node.name,
        self.render_params(function_def.params),
        self.render_ts_type(function_def.return_type.unwrap())
      );
    }
    println!();
  }

  fn print_namespace_details(&self, node: doc::DocNode) {
    let elements = node.namespace_def.unwrap().elements;
    for node in elements {
      self.print_signature(&node, 0);
    }
    println!();
  }

  fn print_function_signature(&self, node: &doc::DocNode, indent: i64) {
    self.print_indent(indent);
    let function_def = node.function_def.clone().unwrap();
    let return_type = function_def.return_type.unwrap();
    println!(
      "function {}({}): {}",
      node.name,
      self.render_params(function_def.params),
      self.render_ts_type(return_type).as_str()
    );
  }

  fn print_class_signature(&self, node: &doc::DocNode, indent: i64) {
    self.print_indent(indent);
    println!("class {}", node.name);
  }

  fn print_variable_signature(&self, node: &doc::DocNode, indent: i64) {
    self.print_indent(indent);
    let variable_def = node.variable_def.clone().unwrap();
    println!(
      "{} {}{}",
      match variable_def.kind {
        swc_ecma_ast::VarDeclKind::Const => "const".to_string(),
        swc_ecma_ast::VarDeclKind::Let => "let".to_string(),
        swc_ecma_ast::VarDeclKind::Var => "var".to_string(),
      },
      node.name,
      if variable_def.ts_type.is_some() {
        format!(": {}", self.render_ts_type(variable_def.ts_type.unwrap()))
      } else {
        "".to_string()
      }
    );
  }

  fn print_enum_signature(&self, node: &doc::DocNode, indent: i64) {
    self.print_indent(indent);
    println!("enum {}", node.name);
  }

  fn print_interface_signature(&self, node: &doc::DocNode, indent: i64) {
    self.print_indent(indent);
    println!("interface {}", node.name);
  }

  fn print_type_alias_signature(&self, node: &doc::DocNode, indent: i64) {
    self.print_indent(indent);
    println!("type {}", node.name);
  }

  fn print_namespace_signature(&self, node: &doc::DocNode, indent: i64) {
    self.print_indent(indent);
    println!("namespace {}", node.name);
  }
}
