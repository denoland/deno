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
use crate::doc::display::{
  display_abstract, display_async, display_generator, Indent, SliceDisplayer,
};
use crate::doc::DocNodeKind;
use std::fmt::{Display, Formatter, Result as FmtResult};

pub struct DocPrinter<'a> {
  doc_nodes: &'a [doc::DocNode],
  private: bool,
}

impl<'a> DocPrinter<'a> {
  pub fn new(doc_nodes: &[doc::DocNode], private: bool) -> DocPrinter {
    DocPrinter { doc_nodes, private }
  }

  pub fn format(&self, w: &mut Formatter<'_>) -> FmtResult {
    self.format_(w, self.doc_nodes, 0)
  }

  fn format_(
    &self,
    w: &mut Formatter<'_>,
    doc_nodes: &[doc::DocNode],
    indent: i64,
  ) -> FmtResult {
    let mut sorted = Vec::from(doc_nodes);
    sorted.sort_unstable_by(|a, b| {
      let kind_cmp = self.kind_order(&a.kind).cmp(&self.kind_order(&b.kind));
      if kind_cmp == core::cmp::Ordering::Equal {
        a.name.cmp(&b.name)
      } else {
        kind_cmp
      }
    });

    for node in &sorted {
      write!(
        w,
        "{}",
        colors::italic_gray(&format!(
          "Defined in {}:{}:{} \n\n",
          node.location.filename, node.location.line, node.location.col
        ))
      )?;

      self.format_signature(w, &node, indent)?;

      let js_doc = &node.js_doc;
      if let Some(js_doc) = js_doc {
        self.format_jsdoc(w, js_doc, indent + 1)?;
      }
      writeln!(w)?;

      match node.kind {
        DocNodeKind::Class => self.format_class(w, node)?,
        DocNodeKind::Enum => self.format_enum(w, node)?,
        DocNodeKind::Interface => self.format_interface(w, node)?,
        DocNodeKind::Namespace => self.format_namespace(w, node)?,
        _ => {}
      }
    }

    Ok(())
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
      DocNodeKind::Import => 7,
    }
  }

  fn format_signature(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
    indent: i64,
  ) -> FmtResult {
    match node.kind {
      DocNodeKind::Function => self.format_function_signature(w, node, indent),
      DocNodeKind::Variable => self.format_variable_signature(w, node, indent),
      DocNodeKind::Class => self.format_class_signature(w, node, indent),
      DocNodeKind::Enum => self.format_enum_signature(w, node, indent),
      DocNodeKind::Interface => {
        self.format_interface_signature(w, node, indent)
      }
      DocNodeKind::TypeAlias => {
        self.format_type_alias_signature(w, node, indent)
      }
      DocNodeKind::Namespace => {
        self.format_namespace_signature(w, node, indent)
      }
      DocNodeKind::Import => Ok(()),
    }
  }

  // TODO(SyrupThinker) this should use a JSDoc parser
  fn format_jsdoc(
    &self,
    w: &mut Formatter<'_>,
    jsdoc: &str,
    indent: i64,
  ) -> FmtResult {
    for line in jsdoc.lines() {
      writeln!(w, "{}{}", Indent(indent), colors::gray(&line))?;
    }

    Ok(())
  }

  fn format_class(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
  ) -> FmtResult {
    let class_def = node.class_def.as_ref().unwrap();
    for node in &class_def.constructors {
      writeln!(w, "{}{}", Indent(1), node,)?;
      if let Some(js_doc) = &node.js_doc {
        self.format_jsdoc(w, &js_doc, 2)?;
      }
    }
    for node in class_def.properties.iter().filter(|node| {
      self.private
        || node
          .accessibility
          .unwrap_or(swc_ecmascript::ast::Accessibility::Public)
          != swc_ecmascript::ast::Accessibility::Private
    }) {
      writeln!(w, "{}{}", Indent(1), node,)?;
      if let Some(js_doc) = &node.js_doc {
        self.format_jsdoc(w, &js_doc, 2)?;
      }
    }
    for index_sign_def in &class_def.index_signatures {
      writeln!(w, "{}{}", Indent(1), index_sign_def)?;
    }
    for node in class_def.methods.iter().filter(|node| {
      self.private
        || node
          .accessibility
          .unwrap_or(swc_ecmascript::ast::Accessibility::Public)
          != swc_ecmascript::ast::Accessibility::Private
    }) {
      writeln!(w, "{}{}", Indent(1), node,)?;
      if let Some(js_doc) = &node.js_doc {
        self.format_jsdoc(w, js_doc, 2)?;
      }
    }
    writeln!(w)
  }

  fn format_enum(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
  ) -> FmtResult {
    let enum_def = node.enum_def.as_ref().unwrap();
    for member in &enum_def.members {
      writeln!(w, "{}{}", Indent(1), colors::bold(&member.name))?;
    }
    writeln!(w)
  }

  fn format_interface(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
  ) -> FmtResult {
    let interface_def = node.interface_def.as_ref().unwrap();

    for property_def in &interface_def.properties {
      writeln!(w, "{}{}", Indent(1), property_def)?;
      if let Some(js_doc) = &property_def.js_doc {
        self.format_jsdoc(w, js_doc, 2)?;
      }
    }
    for method_def in &interface_def.methods {
      writeln!(w, "{}{}", Indent(1), method_def)?;
      if let Some(js_doc) = &method_def.js_doc {
        self.format_jsdoc(w, js_doc, 2)?;
      }
    }
    for index_sign_def in &interface_def.index_signatures {
      writeln!(w, "{}{}", Indent(1), index_sign_def)?;
    }
    writeln!(w)
  }

  fn format_namespace(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
  ) -> FmtResult {
    let elements = &node.namespace_def.as_ref().unwrap().elements;
    for node in elements {
      self.format_signature(w, &node, 1)?;
      if let Some(js_doc) = &node.js_doc {
        self.format_jsdoc(w, js_doc, 2)?;
      }
    }
    writeln!(w)
  }

  fn format_class_signature(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
    indent: i64,
  ) -> FmtResult {
    let class_def = node.class_def.as_ref().unwrap();
    write!(
      w,
      "{}{}{} {}",
      Indent(indent),
      display_abstract(class_def.is_abstract),
      colors::magenta("class"),
      colors::bold(&node.name),
    )?;
    if !class_def.type_params.is_empty() {
      write!(
        w,
        "<{}>",
        SliceDisplayer::new(&class_def.type_params, ", ", false)
      )?;
    }

    if let Some(extends) = &class_def.extends {
      write!(w, " {} {}", colors::magenta("extends"), extends)?;
    }
    if !class_def.super_type_params.is_empty() {
      write!(
        w,
        "<{}>",
        SliceDisplayer::new(&class_def.super_type_params, ", ", false)
      )?;
    }

    if !class_def.implements.is_empty() {
      write!(
        w,
        " {} {}",
        colors::magenta("implements"),
        SliceDisplayer::new(&class_def.implements, ", ", false)
      )?;
    }

    writeln!(w)
  }

  fn format_enum_signature(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
    indent: i64,
  ) -> FmtResult {
    writeln!(
      w,
      "{}{} {}",
      Indent(indent),
      colors::magenta("enum"),
      colors::bold(&node.name)
    )
  }

  fn format_function_signature(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
    indent: i64,
  ) -> FmtResult {
    let function_def = node.function_def.as_ref().unwrap();
    write!(
      w,
      "{}{}{}{} {}",
      Indent(indent),
      display_async(function_def.is_async),
      colors::magenta("function"),
      display_generator(function_def.is_generator),
      colors::bold(&node.name)
    )?;
    if !function_def.type_params.is_empty() {
      write!(
        w,
        "<{}>",
        SliceDisplayer::new(&function_def.type_params, ", ", false)
      )?;
    }
    write!(
      w,
      "({})",
      SliceDisplayer::new(&function_def.params, ", ", false)
    )?;
    if let Some(return_type) = &function_def.return_type {
      write!(w, ": {}", return_type)?;
    }
    writeln!(w)
  }

  fn format_interface_signature(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
    indent: i64,
  ) -> FmtResult {
    let interface_def = node.interface_def.as_ref().unwrap();
    write!(
      w,
      "{}{} {}",
      Indent(indent),
      colors::magenta("interface"),
      colors::bold(&node.name)
    )?;

    if !interface_def.type_params.is_empty() {
      write!(
        w,
        "<{}>",
        SliceDisplayer::new(&interface_def.type_params, ", ", false)
      )?;
    }

    if !interface_def.extends.is_empty() {
      write!(
        w,
        " {} {}",
        colors::magenta("extends"),
        SliceDisplayer::new(&interface_def.extends, ", ", false)
      )?;
    }

    writeln!(w)
  }

  fn format_type_alias_signature(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
    indent: i64,
  ) -> FmtResult {
    let type_alias_def = node.type_alias_def.as_ref().unwrap();
    write!(
      w,
      "{}{} {}",
      Indent(indent),
      colors::magenta("type"),
      colors::bold(&node.name),
    )?;

    if !type_alias_def.type_params.is_empty() {
      write!(
        w,
        "<{}>",
        SliceDisplayer::new(&type_alias_def.type_params, ", ", false)
      )?;
    }

    writeln!(w, " = {}", type_alias_def.ts_type)
  }

  fn format_namespace_signature(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
    indent: i64,
  ) -> FmtResult {
    writeln!(
      w,
      "{}{} {}",
      Indent(indent),
      colors::magenta("namespace"),
      colors::bold(&node.name)
    )
  }

  fn format_variable_signature(
    &self,
    w: &mut Formatter<'_>,
    node: &doc::DocNode,
    indent: i64,
  ) -> FmtResult {
    let variable_def = node.variable_def.as_ref().unwrap();
    write!(
      w,
      "{}{} {}",
      Indent(indent),
      colors::magenta(match variable_def.kind {
        swc_ecmascript::ast::VarDeclKind::Const => "const",
        swc_ecmascript::ast::VarDeclKind::Let => "let",
        swc_ecmascript::ast::VarDeclKind::Var => "var",
      }),
      colors::bold(&node.name),
    )?;
    if let Some(ts_type) = &variable_def.ts_type {
      write!(w, ": {}", ts_type)?;
    }
    writeln!(w)
  }
}

impl<'a> Display for DocPrinter<'a> {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    self.format(f)
  }
}
