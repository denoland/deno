// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(nayeemrmn): Move to `cli/lsp/tsc/mod.rs`.

use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::future::Shared;
use deno_core::serde_json::json;
use deno_path_util::url_to_file_path;
use indexmap::IndexMap;
use indexmap::IndexSet;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::documents::DocumentModule;
use super::language_server::StateSnapshot;
use super::tsc;
use crate::lsp::analysis::TsFixActionCollector;
use crate::lsp::config::CodeLensSettings;
use crate::lsp::diagnostics::ts_json_to_diagnostics;
use crate::lsp::documents::Document;
use crate::lsp::language_server;
use crate::lsp::logging::lsp_warn;
use crate::lsp::performance::Performance;
use crate::lsp::refactor;
use crate::lsp::tsc::{TsServer, file_text_changes_to_workspace_edit};
use crate::lsp::tsc_go::TsGoServer;

#[derive(Debug)]
pub enum TsModServer {
  Js(TsServer),
  Go(TsGoServer),
}

impl TsModServer {
  pub fn new(performance: Arc<Performance>) -> Self {
    if std::env::var("DENO_UNSTABLE_TSGO_LSP").is_ok() {
      TsModServer::Go(TsGoServer::new(performance))
    } else {
      TsModServer::Js(TsServer::new(performance))
    }
  }

  pub fn is_started(&self) -> bool {
    match self {
      TsModServer::Js(ts_server) => ts_server.is_started(),
      TsModServer::Go(ts_server) => ts_server.is_started(),
    }
  }

  pub async fn provide_diagnostics(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Vec<lsp::Diagnostic>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let mut ts_diagnostics = ts_server
          .get_diagnostics(snapshot.clone(), module, token)
          .await?;
        let suggestion_actions_settings = snapshot
          .config
          .language_settings_for_specifier(&module.specifier)
          .map(|s| s.suggestion_actions.clone())
          .unwrap_or_default();
        if !suggestion_actions_settings.enabled {
          ts_diagnostics.retain(|d| {
            d.category != crate::tsc::DiagnosticCategory::Suggestion
              // Still show deprecated and unused diagnostics.
              // https://github.com/microsoft/vscode/blob/ce50bd4876af457f64d83cfd956bc916535285f4/extensions/typescript-language-features/src/languageFeatures/diagnostics.ts#L113-L114
              || d.reports_deprecated == Some(true)
              || d.reports_unnecessary == Some(true)
          });
        }
        Ok(ts_json_to_diagnostics(
          ts_diagnostics,
          module,
          &snapshot.document_modules,
        ))
      }
      TsModServer::Go(ts_server) => {
        let report = ts_server
          .provide_diagnostics(module, snapshot, token)
          .await?;
        let lsp::DocumentDiagnosticReport::Full(report) = report else {
          unreachable!(
            "tsgo currently always returns a full diagnostics report"
          );
        };
        Ok(report.full_document_diagnostic_report.items)
      }
    }
  }

  pub async fn provide_references(
    &self,
    document: &Document,
    module: &DocumentModule,
    position: lsp::Position,
    context: lsp::ReferenceContext,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::Location>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let mut locations = IndexSet::new();
        for module in snapshot
          .document_modules
          .get_or_temp_modules_by_compiler_options_key(document)
          .into_values()
        {
          if token.is_cancelled() {
            return Err(anyhow!("request cancelled"));
          }
          let symbols = ts_server
            .find_references(
              snapshot.clone(),
              &module,
              module.line_index.offset_tsc(position)?,
              token,
            )
            .await
            .inspect_err(|err| {
              if !err.to_string().contains("Could not find source file") {
                lsp_warn!(
                  "Unable to get references from TypeScript: {:#}\nScope: {}",
                  err,
                  module.scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
                );
              }
            })
            .unwrap_or_default();
          for reference in symbols.iter().flatten().flat_map(|s| &s.references)
          {
            if token.is_cancelled() {
              return Err(anyhow!("request cancelled"));
            }
            if !context.include_declaration && reference.is_definition {
              continue;
            }
            let Some(location) =
              reference.entry.to_location(&module, &snapshot)
            else {
              continue;
            };
            locations.insert(location);
          }
        }
        let locations = if locations.is_empty() {
          None
        } else {
          Some(locations.into_iter().collect())
        };
        Ok(locations)
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_references(module, position, context, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_code_lenses(
    &self,
    module: &DocumentModule,
    settings: &CodeLensSettings,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CodeLens>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        if !settings.implementations && !settings.references {
          return Ok(None);
        }
        let navigation_tree = ts_server
          .get_navigation_tree(snapshot, module, token)
          .await
          .map_err(|err| {
            anyhow!(
              "Error getting navigation tree for \"{}\": {:#}",
              &module.specifier,
              err,
            )
          })?;
        let code_lenses = crate::lsp::code_lens::collect_tsc(
          &module.uri,
          settings,
          module.line_index.clone(),
          &navigation_tree,
          token,
        )?;
        if code_lenses.is_empty() {
          Ok(None)
        } else {
          Ok(Some(code_lenses))
        }
      }
      TsModServer::Go(ts_server) => {
        ts_server.provide_code_lenses(module, snapshot, token).await
      }
    }
  }

  pub async fn provide_document_symbols(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::DocumentSymbolResponse>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let navigation_tree = ts_server
          .get_navigation_tree(snapshot, module, token)
          .await
          .map_err(|err| {
            anyhow!(
              "Error getting navigation tree for \"{}\": {:#}",
              &module.specifier,
              err,
            )
          })?;
        let response = if let Some(child_items) = &navigation_tree.child_items {
          let mut document_symbols = Vec::new();
          for item in child_items {
            if token.is_cancelled() {
              return Err(anyhow!("request cancelled"));
            }
            item.collect_document_symbols(
              module.line_index.clone(),
              &mut document_symbols,
            );
          }
          Some(lsp::DocumentSymbolResponse::Nested(document_symbols))
        } else {
          None
        };
        Ok(response)
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_document_symbols(module, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_hover(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::Hover>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let position = module.line_index.offset_tsc(position)?;
        let quick_info = ts_server
          .get_quick_info(snapshot.clone(), module, position, token)
          .await?;
        Ok(quick_info.map(|qi| qi.to_hover(module, &snapshot)))
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_hover(module, position, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_code_actions(
    &self,
    module: &DocumentModule,
    range: lsp::Range,
    context: &lsp::CodeActionContext,
    file_diagnostics: Shared<impl Future<Output = Arc<Vec<lsp::Diagnostic>>>>,
    has_deno_code_actions: bool,
    language_server: &language_server::Inner,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::CodeActionResponse>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let supported_code_fixes =
          ts_server.get_supported_code_fixes(snapshot.clone()).await?;
        let fixable_diagnostics = context.diagnostics.iter().filter(|d| {
          d.source.as_deref() == Some("deno-ts")
            && match &d.code {
              Some(lsp::NumberOrString::String(code)) => {
                supported_code_fixes.contains(code)
              }
              Some(lsp::NumberOrString::Number(code)) => {
                supported_code_fixes.contains(&code.to_string())
              }
              _ => false,
            }
        });
        let mut collector = TsFixActionCollector::default();
        let file_diagnostics = file_diagnostics.await;
        for diagnostic in fixable_diagnostics {
          if token.is_cancelled() {
            return Err(anyhow!("request cancelled"));
          }
          let code = match &diagnostic.code {
            Some(lsp::NumberOrString::String(code)) => match code.parse() {
              Ok(c) => c,
              Err(e) => {
                lsp_warn!("Invalid diagnostic code {code}: {e}");
                continue;
              }
            },
            Some(lsp::NumberOrString::Number(code)) => *code,
            _ => {
              lsp_warn!("Missing diagnostic code for: {:#?}", diagnostic);
              continue;
            }
          };
          let fix_actions = ts_server
            .get_code_fixes(
              snapshot.clone(),
              module,
              module.line_index.offset_tsc(diagnostic.range.start)?
                ..module.line_index.offset_tsc(diagnostic.range.end)?,
              vec![code],
              token,
            )
            .await
            .unwrap_or_else(|err| {
              // sometimes tsc reports errors when retrieving code actions
              // because they don't reflect the current state of the document
              // so we will log them to the output, but we won't send an error
              // message back to the client.
              if !token.is_cancelled() {
                lsp_warn!(
                  "Unable to get code fixes from TypeScript: {:#}",
                  err
                );
              }
              vec![]
            });
          for fix_action in fix_actions {
            if token.is_cancelled() {
              return Err(anyhow!("request cancelled"));
            }
            collector
              .add_ts_fix_action(
                &fix_action,
                diagnostic,
                module,
                language_server,
              )
              .map_err(|err| anyhow!("Unable to convert fix: {:#}", err))?;
            if collector.is_fix_all_action(
              &fix_action,
              diagnostic,
              &file_diagnostics,
            ) {
              collector.add_ts_fix_all_action(&fix_action, module, diagnostic);
            }
          }
        }

        let mut actions = collector
          .into_code_actions(has_deno_code_actions)
          .map(lsp::CodeActionOrCommand::CodeAction)
          .collect::<Vec<_>>();

        let only = context
          .only
          .as_ref()
          .and_then(|o| o.first())
          .map(|v| v.as_str().to_owned())
          .unwrap_or_default();
        let refactor_infos = ts_server
          .get_applicable_refactors(
            snapshot.clone(),
            module,
            module.line_index.offset_tsc(range.start)?
              ..module.line_index.offset_tsc(range.end)?,
            context.trigger_kind,
            only,
            token,
          )
          .await
          .map_err(|err| {
            anyhow!("Unable to get refactor info from TypeScript: {:#}", err)
          })?;
        let refactor_actions = refactor_infos
          .into_iter()
          .map(|refactor_info| {
            refactor_info
              .to_code_actions(&module.uri, &range, token)
              .map_err(|err| {
                anyhow!("Unable to convert refactor info: {:#}", err)
              })
          })
          .collect::<Result<Vec<_>, _>>()?
          .into_iter()
          .flatten();
        actions.extend(
          refactor::prune_invalid_actions(refactor_actions, 5)
            .into_iter()
            .map(lsp::CodeActionOrCommand::CodeAction),
        );

        if !snapshot.config.client_provided_organize_imports_capable()
          && context.only.as_ref().is_none_or(|o| {
            o.contains(&lsp::CodeActionKind::SOURCE_ORGANIZE_IMPORTS)
          })
        {
          let document_has_errors = context.diagnostics.iter().any(|d| {
            // Assume diagnostics without a severity are errors
            d.severity
              .is_none_or(|s| s == lsp::DiagnosticSeverity::ERROR)
          });
          let organize_imports_edit = ts_server
            .organize_imports(
              snapshot.clone(),
              module,
              document_has_errors,
              token,
            )
            .await
            .map_err(|err| {
              anyhow!(
                "Unable to get organize imports edit from TypeScript: {:#}",
                err
              )
            })?;
          if !organize_imports_edit.is_empty() {
            let changes_with_modules = organize_imports_edit
              .iter()
              .map(|c| (c, module))
              .collect::<IndexMap<_, _>>();
            actions.push(lsp::CodeActionOrCommand::CodeAction(
              lsp::CodeAction {
                title: "Organize imports".to_string(),
                kind: Some(lsp::CodeActionKind::SOURCE_ORGANIZE_IMPORTS),
                edit: file_text_changes_to_workspace_edit(
                  changes_with_modules,
                  language_server,
                  token,
                )?,
                data: Some(json!({ "uri": &module.uri})),
                ..Default::default()
              },
            ));
          }
        }

        Ok(Some(actions))
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_code_actions(module, range, context, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_document_highlights(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::DocumentHighlight>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let highlights = ts_server
          .get_document_highlights(
            snapshot,
            module,
            module.line_index.offset_tsc(position)?,
            token,
          )
          .await?;
        highlights
          .map(|highlights| {
            highlights
              .into_iter()
              .map(|dh| {
                dh.to_highlight(module.line_index.clone(), token).map_err(
                  |err| {
                    anyhow!("Unable to convert document highlights: {:#}", err)
                  },
                )
              })
              .collect::<Result<Vec<_>, _>>()
              .map(|s| s.into_iter().flatten().collect())
          })
          .transpose()
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_document_highlights(module, position, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_definition(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::GotoDefinitionResponse>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let definition_info = ts_server
          .get_definition(
            snapshot.clone(),
            module,
            module.line_index.offset_tsc(position)?,
            token,
          )
          .await?;
        definition_info
          .map(|definition_info| {
            definition_info
              .to_definition(&module, &snapshot, token)
              .map_err(|err| {
                anyhow!("Unable to convert definition info: {:#}", err)
              })
          })
          .transpose()
          .map(|d| d.flatten())
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_definition(module, position, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_type_definition(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoTypeDefinitionResponse>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let definition_info = ts_server
          .get_type_definition(
            snapshot.clone(),
            &module,
            module.line_index.offset_tsc(position)?,
            token,
          )
          .await?;
        definition_info
          .map(|definition_info| {
            let mut location_links = Vec::new();
            for info in definition_info {
              if token.is_cancelled() {
                return Err(anyhow!("request cancelled"));
              }
              if let Some(link) = info.document_span.to_link(&module, &snapshot)
              {
                location_links.push(link);
              }
            }
            Ok(lsp::request::GotoTypeDefinitionResponse::Link(
              location_links,
            ))
          })
          .transpose()
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_type_definition(module, position, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_completion(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: Option<&lsp::CompletionContext>,
    language_server: &language_server::Inner,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::CompletionResponse>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let position = module.line_index.offset_tsc(position)?;
        let completion_info = ts_server
          .get_completions(
            snapshot.clone(),
            module,
            position,
            context.as_ref().and_then(|c| c.trigger_character.clone()),
            context.as_ref().map(|c| c.trigger_kind.into()),
            token,
          )
          .await
          .unwrap_or_else(|err| {
            if !token.is_cancelled() {
              lsp_warn!(
                "Unable to get completion info from TypeScript: {:#}",
                err
              );
            }
            None
          });
        completion_info
          .map(|completion_info| {
            completion_info
              .as_completion_response(
                module.line_index.clone(),
                &snapshot
                  .config
                  .language_settings_for_specifier(&module.specifier)
                  .cloned()
                  .unwrap_or_default()
                  .suggest,
                module,
                position,
                language_server,
                token,
              )
              .map_err(|err| {
                anyhow!("Unable to convert completion info: {:#}", err)
              })
          })
          .transpose()
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_completion(module, position, context, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_implementations(
    &self,
    document: &Document,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoImplementationResponse>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let mut implementations_with_modules = IndexMap::new();
        for module in snapshot
          .document_modules
          .get_or_temp_modules_by_compiler_options_key(&document)
          .into_values()
        {
          if token.is_cancelled() {
            return Err(anyhow!("request cancelled"));
          }
          let implementations = ts_server
            .get_implementations(
              snapshot.clone(),
              &module,
              module
                .line_index
                .offset_tsc(position)?,
              token,
            )
            .await
            .inspect_err(|err| {
              if !err.to_string().contains("Could not find source file") {
                lsp_warn!(
                  "Unable to get implementation locations from TypeScript: {:#}\nScope: {}",
                  err,
                  module.scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
                );
              }
            })
            .unwrap_or_default();
          if let Some(implementations) = implementations {
            implementations_with_modules
              .extend(implementations.into_iter().map(|i| (i, module.clone())))
          }
        }
        let links = implementations_with_modules
          .iter()
          .flat_map(|(i, module)| {
            if token.is_cancelled() {
              return Some(Err(anyhow!("request cancelled")));
            }
            Some(Ok(i.to_link(module, &snapshot)?))
          })
          .collect::<Result<Vec<_>, _>>()?;
        if links.is_empty() {
          Ok(None)
        } else {
          Ok(Some(lsp::GotoDefinitionResponse::Link(
            links.into_iter().collect(),
          )))
        }
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_implementations(module, position, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_folding_range(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::FoldingRange>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let outlining_spans = ts_server
          .get_outlining_spans(snapshot.clone(), &module, token)
          .await?;
        if !outlining_spans.is_empty() {
          let folding_ranges = outlining_spans
            .iter()
            .map(|span| {
              if token.is_cancelled() {
                return Err(anyhow!("request cancelled"));
              }
              Ok(span.to_folding_range(
                module.line_index.clone(),
                module.text.as_bytes(),
                snapshot.config.line_folding_only_capable(),
              ))
            })
            .collect::<Result<Vec<_>, _>>()?;
          Ok(Some(folding_ranges))
        } else {
          Ok(None)
        }
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_folding_range(module, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_call_hierarchy_incoming_calls(
    &self,
    document: &Document,
    module: &DocumentModule,
    item: &lsp::CallHierarchyItem,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyIncomingCall>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let mut incoming_calls_with_modules = IndexMap::new();
        for module in snapshot
          .document_modules
          .get_or_temp_modules_by_compiler_options_key(&document)
          .into_values()
        {
          if token.is_cancelled() {
            return Err(anyhow!("request cancelled"));
          }
          let incoming_calls = ts_server
            .provide_call_hierarchy_incoming_calls(
              snapshot.clone(),
              &module,
              module.line_index.offset_tsc(item.selection_range.start)?,
              token,
            )
            .await
            .inspect_err(|err| {
              lsp_warn!(
                "Unable to get incoming calls from TypeScript: {:#}\nScope: {}",
                err,
                module.scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
              );
            })
            .unwrap_or_default();
          incoming_calls_with_modules
            .extend(incoming_calls.into_iter().map(|c| (c, module.clone())));
        }
        let root_path = snapshot
          .config
          .root_url()
          .and_then(|s| url_to_file_path(s).ok());
        let incoming_calls = incoming_calls_with_modules
          .iter()
          .flat_map(|(c, module)| {
            if token.is_cancelled() {
              return Some(Err(anyhow!("request cancelled")));
            }
            Some(Ok(c.try_resolve_call_hierarchy_incoming_call(
              module,
              &snapshot,
              root_path.as_deref(),
            )?))
          })
          .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(incoming_calls))
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_call_hierarchy_incoming_calls(module, item, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_call_hierarchy_outgoing_calls(
    &self,
    module: &DocumentModule,
    item: &lsp::CallHierarchyItem,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyOutgoingCall>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let outgoing_calls = ts_server
          .provide_call_hierarchy_outgoing_calls(
            snapshot.clone(),
            &module,
            module.line_index.offset_tsc(item.selection_range.start)?,
            token,
          )
          .await?;
        let root_path = snapshot
          .config
          .root_url()
          .and_then(|s| url_to_file_path(s).ok());
        let outgoing_calls = outgoing_calls
          .iter()
          .flat_map(|c| {
            if token.is_cancelled() {
              return Some(Err(anyhow!("request cancelled")));
            }
            Some(Ok(c.try_resolve_call_hierarchy_outgoing_call(
              &module,
              &snapshot,
              root_path.as_deref(),
            )?))
          })
          .collect::<Result<_, _>>()?;
        Ok(Some(outgoing_calls))
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_call_hierarchy_outgoing_calls(module, item, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_prepare_call_hierarchy(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyItem>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let items = ts_server
          .prepare_call_hierarchy(
            snapshot.clone(),
            module,
            module.line_index.offset_tsc(position)?,
            token,
          )
          .await?;
        items
          .map(|items| {
            let items = items.into_vec();
            let root_path = snapshot
              .config
              .root_url()
              .and_then(|s| url_to_file_path(s).ok());
            let items = items
              .into_iter()
              .flat_map(|item| {
                if token.is_cancelled() {
                  return Some(Err(anyhow!("request cancelled")));
                }
                let item = item.try_resolve_call_hierarchy_item(
                  module,
                  &snapshot,
                  root_path.as_deref(),
                )?;
                Some(Ok(item))
              })
              .collect::<Result<Vec<_>, _>>()?;
            Ok(items)
          })
          .transpose()
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_prepare_call_hierarchy(module, position, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_rename(
    &self,
    document: &Document,
    module: &DocumentModule,
    position: lsp::Position,
    new_name: &str,
    language_server: &language_server::Inner,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let mut locations_with_modules = IndexMap::new();
        for module in snapshot
          .document_modules
          .get_or_temp_modules_by_compiler_options_key(&document)
          .into_values()
        {
          if token.is_cancelled() {
            return Err(anyhow!("request cancelled"));
          }
          let locations = ts_server
          .find_rename_locations(
            snapshot.clone(),
            &module,
            module
              .line_index
              .offset_tsc(position)?,
            token,
          )
          .await
          .inspect_err(|err| {
            if !err.to_string().contains("Could not find source file") {
              lsp_warn!(
                "Unable to get rename locations from TypeScript: {:#}\nScope: {}",
                err,
                module.scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
              );
            }
          })
          .unwrap_or_default();
          if let Some(locations) = locations {
            locations_with_modules
              .extend(locations.into_iter().map(|l| (l, module.clone())));
          }
        }
        if locations_with_modules.is_empty() {
          Ok(None)
        } else {
          let workspace_edit =
            tsc::RenameLocation::collect_into_workspace_edit(
              locations_with_modules,
              new_name,
              language_server,
              token,
            )
            .map_err(|err| {
              anyhow!("Unable to covert rename locations: {:#}", err)
            })?;
          Ok(Some(workspace_edit))
        }
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_rename(module, position, new_name, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_selection_ranges(
    &self,
    module: &DocumentModule,
    positions: &[lsp::Position],
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SelectionRange>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let mut selection_ranges = Vec::with_capacity(positions.len());
        for &position in positions {
          if token.is_cancelled() {
            return Err(anyhow!("request cancelled"));
          }
          let selection_range = ts_server
            .get_smart_selection_range(
              snapshot.clone(),
              &module,
              module.line_index.offset_tsc(position)?,
              token,
            )
            .await?;
          selection_ranges.push(
            selection_range.to_selection_range(module.line_index.clone()),
          );
        }
        Ok(Some(selection_ranges))
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_selection_ranges(module, positions, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_signature_help(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: Option<&lsp::SignatureHelpContext>,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::SignatureHelp>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let options = if let Some(context) = context {
          tsc::SignatureHelpItemsOptions {
            trigger_reason: Some(tsc::SignatureHelpTriggerReason {
              kind: context.trigger_kind.clone().into(),
              trigger_character: context.trigger_character.clone(),
            }),
          }
        } else {
          tsc::SignatureHelpItemsOptions {
            trigger_reason: None,
          }
        };
        let signature_help = ts_server
          .get_signature_help_items(
            snapshot.clone(),
            module,
            module.line_index.offset_tsc(position)?,
            options,
            token,
          )
          .await?;
        signature_help
          .map(|signature_help| {
            signature_help.into_signature_help(&module, &snapshot, token)
          })
          .transpose()
          .map_err(|err| {
            anyhow!("Unable to convert signature help items: {:#}", err)
          })
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_signature_help(module, position, context, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_inlay_hint(
    &self,
    module: &DocumentModule,
    range: lsp::Range,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::InlayHint>>, AnyError> {
    match self {
      TsModServer::Js(ts_server) => {
        let text_span =
          tsc::TextSpan::from_range(range, module.line_index.clone()).map_err(
            |err| {
              anyhow!("Failed to convert range to tsc text span: {:#}", err)
            },
          )?;
        let mut inlay_hints = ts_server
          .provide_inlay_hints(snapshot.clone(), &module, text_span, token)
          .await;
        // Silence tsc debug failures.
        // See https://github.com/denoland/deno/issues/30455.
        // TODO(nayeemrmn): Keeps tabs on whether this is still necessary.
        if let Err(err) = &inlay_hints
          && err.to_string().contains("Debug Failure")
        {
          lsp_warn!("Unable to get inlay hints from TypeScript: {:#}", err);
          inlay_hints = Ok(None)
        }
        inlay_hints?
          .map(|inlay_hints| {
            inlay_hints
              .into_iter()
              .map(|inlay_hint| {
                if token.is_cancelled() {
                  return Err(anyhow!("request cancelled"));
                }
                Ok(inlay_hint.to_lsp(&module, &snapshot))
              })
              .collect()
          })
          .transpose()
      }
      TsModServer::Go(ts_server) => {
        ts_server
          .provide_inlay_hint(module, range, snapshot, token)
          .await
      }
    }
  }
}
