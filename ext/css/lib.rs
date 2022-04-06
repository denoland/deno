// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod properties;
mod values;

use std::path::PathBuf;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::Extension;
use parcel_css::traits::ToCss;
use serde::Serialize;

use crate::properties::Property;

pub fn init() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/css",
      "00_cssom.js",
    ))
    .ops(vec![op_css_parse_rule::decl()])
    .build()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
enum CssRule {
  Style(CssStyleRule),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CssStyleRule {
  selector: String,
  style: CssStyleDeclaration,
}

impl<'i> From<parcel_css::rules::style::StyleRule<'i>> for CssStyleRule {
  fn from(rule: parcel_css::rules::style::StyleRule<'i>) -> Self {
    Self {
      selector: rule.selectors.to_string(),
      style: rule.declarations.into(),
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CssStyleDeclaration {
  text: String,
  properties: Vec<(Property, bool)>,
}

impl<'i> From<parcel_css::declaration::DeclarationBlock<'i>>
  for CssStyleDeclaration
{
  fn from(declaration: parcel_css::declaration::DeclarationBlock<'i>) -> Self {
    let text = declaration
      .to_css_string(parcel_css::stylesheet::PrinterOptions {
        minify: false,
        source_map: None,
        targets: None,
        analyze_dependencies: false,
        pseudo_classes: None,
      })
      .unwrap(); // TODO(lucacasonato): is this unwrap safe?
    let mut properties = Vec::with_capacity(
      declaration.declarations.len() + declaration.important_declarations.len(),
    );
    for property in declaration.declarations {
      properties.push((property.into(), false));
    }
    for property in declaration.important_declarations {
      properties.push((property.into(), true));
    }
    Self { text, properties }
  }
}

#[op]
fn op_css_parse_rule(code: String) -> Result<CssRule, AnyError> {
  let options = parcel_css::stylesheet::ParserOptions::default();
  let rule = parcel_css::rules::CssRule::parse_string(&code, options)
    .map_err(|err| anyhow!("{}", err))?;
  let rule = match rule {
    parcel_css::rules::CssRule::Style(style) => CssRule::Style(style.into()),
    _ => todo!(),
  };
  Ok(rule)
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_css.d.ts")
}
