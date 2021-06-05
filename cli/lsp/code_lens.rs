// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::language_server;
use super::tsc;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use regex::Regex;
use std::cell::RefCell;
use std::rc::Rc;

lazy_static::lazy_static! {
  static ref ABSTRACT_MODIFIER: Regex = Regex::new(r"\babstract\b").unwrap();
  static ref EXPORT_MODIFIER: Regex = Regex::new(r"\bexport\b").unwrap();
}

#[derive(Debug, Deserialize, Serialize)]
pub enum CodeLensSource {
  #[serde(rename = "implementations")]
  Implementations,
  #[serde(rename = "references")]
  References,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensData {
  pub source: CodeLensSource,
  pub specifier: ModuleSpecifier,
}

async fn resolve_implementation_code_lens(
  code_lens: lsp::CodeLens,
  data: CodeLensData,
  language_server: &mut language_server::Inner,
) -> Result<lsp::CodeLens, AnyError> {
  let line_index = language_server
    .get_line_index_sync(&data.specifier)
    .unwrap();
  let req = tsc::RequestMethod::GetImplementation((
    data.specifier.clone(),
    line_index.offset_tsc(code_lens.range.start)?,
  ));
  let snapshot = language_server.snapshot()?;
  let maybe_implementations: Option<Vec<tsc::ImplementationLocation>> =
    language_server.ts_server.request(snapshot, req).await?;
  if let Some(implementations) = maybe_implementations {
    let mut locations = Vec::new();
    for implementation in implementations {
      let implementation_specifier =
        resolve_url(&implementation.document_span.file_name)?;
      let implementation_location =
        implementation.to_location(&line_index, language_server);
      if !(implementation_specifier == data.specifier
        && implementation_location.range.start == code_lens.range.start)
      {
        locations.push(implementation_location);
      }
    }
    let command = if !locations.is_empty() {
      let title = if locations.len() > 1 {
        format!("{} implementations", locations.len())
      } else {
        "1 implementation".to_string()
      };
      lsp::Command {
        title,
        command: "deno.showReferences".to_string(),
        arguments: Some(vec![
          json!(data.specifier),
          json!(code_lens.range.start),
          json!(locations),
        ]),
      }
    } else {
      lsp::Command {
        title: "0 implementations".to_string(),
        command: "".to_string(),
        arguments: None,
      }
    };
    Ok(lsp::CodeLens {
      range: code_lens.range,
      command: Some(command),
      data: None,
    })
  } else {
    let command = Some(lsp::Command {
      title: "0 implementations".to_string(),
      command: "".to_string(),
      arguments: None,
    });
    Ok(lsp::CodeLens {
      range: code_lens.range,
      command,
      data: None,
    })
  }
}

async fn resolve_references_code_lens(
  code_lens: lsp::CodeLens,
  data: CodeLensData,
  language_server: &mut language_server::Inner,
) -> Result<lsp::CodeLens, AnyError> {
  let line_index = language_server
    .get_line_index_sync(&data.specifier)
    .unwrap();
  let req = tsc::RequestMethod::GetReferences((
    data.specifier.clone(),
    line_index.offset_tsc(code_lens.range.start)?,
  ));
  let snapshot = language_server.snapshot()?;
  let maybe_references: Option<Vec<tsc::ReferenceEntry>> =
    language_server.ts_server.request(snapshot, req).await?;
  if let Some(references) = maybe_references {
    let mut locations = Vec::new();
    for reference in references {
      if reference.is_definition {
        continue;
      }
      let reference_specifier =
        resolve_url(&reference.document_span.file_name)?;
      let line_index =
        language_server.get_line_index(reference_specifier).await?;
      locations.push(reference.to_location(&line_index, language_server));
    }
    let command = if !locations.is_empty() {
      let title = if locations.len() > 1 {
        format!("{} references", locations.len())
      } else {
        "1 reference".to_string()
      };
      lsp::Command {
        title,
        command: "deno.showReferences".to_string(),
        arguments: Some(vec![
          json!(data.specifier),
          json!(code_lens.range.start),
          json!(locations),
        ]),
      }
    } else {
      lsp::Command {
        title: "0 references".to_string(),
        command: "".to_string(),
        arguments: None,
      }
    };
    Ok(lsp::CodeLens {
      range: code_lens.range,
      command: Some(command),
      data: None,
    })
  } else {
    let command = lsp::Command {
      title: "0 references".to_string(),
      command: "".to_string(),
      arguments: None,
    };
    Ok(lsp::CodeLens {
      range: code_lens.range,
      command: Some(command),
      data: None,
    })
  }
}

pub(crate) async fn resolve_code_lens(
  code_lens: lsp::CodeLens,
  language_server: &mut language_server::Inner,
) -> Result<lsp::CodeLens, AnyError> {
  let data: CodeLensData =
    serde_json::from_value(code_lens.data.clone().unwrap())?;
  match data.source {
    CodeLensSource::Implementations => {
      resolve_implementation_code_lens(code_lens, data, language_server).await
    }
    CodeLensSource::References => {
      resolve_references_code_lens(code_lens, data, language_server).await
    }
  }
}

/// Return tsc navigation tree code lenses.
pub(crate) async fn tsc_code_lenses(
  specifier: &ModuleSpecifier,
  language_server: &mut language_server::Inner,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  let workspace_settings = language_server.config.get_workspace_settings();
  let line_index = language_server
    .get_line_index_sync(&specifier)
    .ok_or_else(|| anyhow!("Missing line index."))?;
  let navigation_tree = language_server.get_navigation_tree(specifier).await?;
  let code_lenses = Rc::new(RefCell::new(Vec::new()));
  navigation_tree.walk(&|i, mp| {
    let mut code_lenses = code_lenses.borrow_mut();

    // TSC Implementations Code Lens
    if workspace_settings.code_lens.implementations {
      let source = CodeLensSource::Implementations;
      match i.kind {
        tsc::ScriptElementKind::InterfaceElement => {
          code_lenses.push(i.to_code_lens(&line_index, specifier, &source));
        }
        tsc::ScriptElementKind::ClassElement
        | tsc::ScriptElementKind::MemberFunctionElement
        | tsc::ScriptElementKind::MemberVariableElement
        | tsc::ScriptElementKind::MemberGetAccessorElement
        | tsc::ScriptElementKind::MemberSetAccessorElement => {
          if ABSTRACT_MODIFIER.is_match(&i.kind_modifiers) {
            code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
          }
        }
        _ => (),
      }
    }

    // TSC References Code Lens
    if workspace_settings.code_lens.references {
      let source = CodeLensSource::References;
      if let Some(parent) = &mp {
        if parent.kind == tsc::ScriptElementKind::EnumElement {
          code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
        }
      }
      match i.kind {
        tsc::ScriptElementKind::FunctionElement => {
          if workspace_settings.code_lens.references_all_functions {
            code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
          }
        }
        tsc::ScriptElementKind::ConstElement
        | tsc::ScriptElementKind::LetElement
        | tsc::ScriptElementKind::VariableElement => {
          if EXPORT_MODIFIER.is_match(&i.kind_modifiers) {
            code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
          }
        }
        tsc::ScriptElementKind::ClassElement => {
          if i.text != "<class>" {
            code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
          }
        }
        tsc::ScriptElementKind::InterfaceElement
        | tsc::ScriptElementKind::TypeElement
        | tsc::ScriptElementKind::EnumElement => {
          code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
        }
        tsc::ScriptElementKind::LocalFunctionElement
        | tsc::ScriptElementKind::MemberGetAccessorElement
        | tsc::ScriptElementKind::MemberSetAccessorElement
        | tsc::ScriptElementKind::ConstructorImplementationElement
        | tsc::ScriptElementKind::MemberVariableElement => {
          if let Some(parent) = &mp {
            if parent.spans[0].start != i.spans[0].start {
              match parent.kind {
                tsc::ScriptElementKind::ClassElement
                | tsc::ScriptElementKind::InterfaceElement
                | tsc::ScriptElementKind::TypeElement => {
                  code_lenses.push(i.to_code_lens(
                    &line_index,
                    &specifier,
                    &source,
                  ));
                }
                _ => (),
              }
            }
          }
        }
        _ => (),
      }
    }
  });
  Ok(Rc::try_unwrap(code_lenses).unwrap().into_inner())
}
