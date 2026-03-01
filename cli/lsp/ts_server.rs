// Copyright 2018-2026 the Deno authors. MIT license.

use std::str::FromStr;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::future::Shared;
use deno_core::serde_json::json;
use deno_resolver::deno_json::CompilerOptionsKey;
use indexmap::IndexMap;
use indexmap::IndexSet;
use lsp_types::Uri;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::documents::DocumentModule;
use super::language_server::StateSnapshot;
use super::tsc;
use crate::cache::DenoDir;
use crate::http_util::HttpClientProvider;
use crate::lsp::analysis::TsFixActionCollector;
use crate::lsp::analysis::fix_ts_import_changes_for_file_rename;
use crate::lsp::completions;
use crate::lsp::completions::CompletionItemData;
use crate::lsp::config::CodeLensSettings;
use crate::lsp::config::UpdateImportsOnFileMoveEnabled;
use crate::lsp::diagnostics::ts_json_to_diagnostics;
use crate::lsp::documents::Document;
use crate::lsp::language_server;
use crate::lsp::logging::lsp_warn;
use crate::lsp::performance::Performance;
use crate::lsp::refactor;
use crate::lsp::tsc::TsJsServer;
use crate::lsp::tsc::file_text_changes_to_workspace_edit;
use crate::lsp::tsgo::TsGoServer;
use crate::lsp::urls::uri_to_url;

#[derive(Debug)]
pub enum TsServer {
  Js(TsJsServer),
  Go(TsGoServer),
}

impl TsServer {
  pub fn new(
    performance: Arc<Performance>,
    deno_dir: &DenoDir,
    http_client_provider: &Arc<HttpClientProvider>,
  ) -> Self {
    if std::env::var("DENO_UNSTABLE_TSGO_LSP").is_ok() {
      Self::Go(TsGoServer::new(deno_dir, http_client_provider))
    } else {
      Self::Js(TsJsServer::new(performance))
    }
  }

  pub fn is_started(&self) -> bool {
    match self {
      Self::Js(ts_server) => ts_server.is_started(),
      Self::Go(ts_server) => ts_server.is_started(),
    }
  }

  pub fn project_changed(
    &self,
    documents: &[(Document, super::tsc::ChangeKind)],
    configuration_changed: bool,
    snapshot: Arc<StateSnapshot>,
  ) {
    match self {
      Self::Js(ts_server) => {
        let new_compiler_options_by_key = configuration_changed.then(|| {
          snapshot
            .compiler_options_resolver
            .entries()
            .map(|(k, d)| (k.clone(), d.compiler_options.clone()))
            .collect()
        });
        let new_notebook_keys = configuration_changed.then(|| {
          snapshot
            .document_modules
            .documents
            .cells_by_notebook_uri()
            .keys()
            .map(|u| {
              let compiler_options_key = snapshot
                .compiler_options_resolver
                .entry_for_specifier(&uri_to_url(u))
                .0;
              (u.clone(), compiler_options_key.clone())
            })
            .collect()
        });
        ts_server.project_changed(
          snapshot,
          documents,
          new_compiler_options_by_key,
          new_notebook_keys,
        );
      }
      Self::Go(ts_server) => {
        ts_server.project_changed(documents, configuration_changed, snapshot);
      }
    }
  }

  pub async fn get_ambient_modules(
    &self,
    compiler_options_key: &CompilerOptionsKey,
    notebook_uri: Option<&Arc<Uri>>,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Vec<String>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        ts_server
          .get_ambient_modules(
            snapshot,
            compiler_options_key,
            notebook_uri,
            token,
          )
          .await
      }
      Self::Go(ts_server) => {
        ts_server
          .get_ambient_modules(
            compiler_options_key,
            notebook_uri,
            snapshot,
            token,
          )
          .await
      }
    }
  }

  pub async fn provide_diagnostics(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Vec<lsp::Diagnostic>, AnyError> {
    match self {
      Self::Js(ts_server) => {
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
      Self::Go(ts_server) => {
        let report = ts_server
          .provide_diagnostics(module, &snapshot, token)
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::Location>>, AnyError> {
    match self {
      Self::Js(ts_server) => {
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
            let Some(location) = reference.entry.to_location(&module, snapshot)
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
      Self::Go(ts_server) => {
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
      Self::Js(ts_server) => {
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
      Self::Go(ts_server) => {
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
      Self::Js(ts_server) => {
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
      Self::Go(ts_server) => {
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
      Self::Js(ts_server) => {
        let position = module.line_index.offset_tsc(position)?;
        let quick_info = ts_server
          .get_quick_info(snapshot.clone(), module, position, token)
          .await?;
        Ok(quick_info.map(|qi| qi.to_hover(module, &snapshot)))
      }
      Self::Go(ts_server) => {
        ts_server
          .provide_hover(module, position, snapshot, token)
          .await
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
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
      Self::Js(ts_server) => {
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
                  &snapshot,
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
      Self::Go(ts_server) => {
        ts_server
          .provide_code_actions(module, range, context, &snapshot, token)
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
      Self::Js(ts_server) => {
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
      Self::Go(ts_server) => {
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::GotoDefinitionResponse>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let definition_info = ts_server
          .get_definition(
            snapshot.clone(),
            module,
            module.line_index.offset_tsc(position)?,
            token,
          )
          .await?;
        super::tsc::DocumentSpan::collect_into_goto_definition_response(
          definition_info
            .iter()
            .flat_map(|i| &i.definitions)
            .flatten()
            .map(|i| (&i.document_span, module)),
          snapshot,
          token,
        )
        .map_err(|err| anyhow!("Unable to convert definition info: {:#}", err))
      }
      Self::Go(ts_server) => {
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoTypeDefinitionResponse>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let definition_info = ts_server
          .get_type_definition(
            snapshot.clone(),
            module,
            module.line_index.offset_tsc(position)?,
            token,
          )
          .await?;
        super::tsc::DocumentSpan::collect_into_goto_definition_response(
          definition_info
            .iter()
            .flatten()
            .map(|i| (&i.document_span, module)),
          snapshot,
          token,
        )
        .map_err(|err| {
          anyhow!("Unable to convert type definition info: {:#}", err)
        })
      }
      Self::Go(ts_server) => {
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
      Self::Js(ts_server) => {
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
      Self::Go(ts_server) => {
        ts_server
          .provide_completion(module, position, context, snapshot, token)
          .await
      }
    }
  }

  pub async fn resolve_completion_item(
    &self,
    module: &DocumentModule,
    item: lsp::CompletionItem,
    data: completions::CompletionItemData,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<lsp::CompletionItem, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let CompletionItemData::TsJs(data) = data else {
          return Ok(item);
        };
        match ts_server
          .get_completion_details(
            snapshot.clone(),
            module,
            data.position,
            data.name.clone(),
            data.source.clone(),
            data.data.clone(),
            token,
          )
          .await
        {
          Ok(Some(completion_details)) => completion_details
            .as_completion_item(&item, &data, module, &snapshot)
            .map_err(|err| {
              anyhow!("Unable to convert completion details: {:#}", err)
            }),
          Ok(None) => {
            if !token.is_cancelled() {
              lsp_warn!(
                "Received undefined completion details from TypeScript for item: {:#?}",
                &item,
              );
            }
            Ok(item)
          }
          Err(err) => {
            if !token.is_cancelled() {
              lsp_warn!(
                "Unable to get completion details from TypeScript: {:#}",
                err
              );
            }
            Ok(item)
          }
        }
      }
      Self::Go(ts_server) => {
        let CompletionItemData::TsGo(data) = data else {
          return Ok(item);
        };
        ts_server
          .resolve_completion_item(module, item, data, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_implementations(
    &self,
    document: &Document,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoImplementationResponse>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let mut implementations_with_modules = IndexMap::new();
        for module in snapshot
          .document_modules
          .get_or_temp_modules_by_compiler_options_key(document)
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
        super::tsc::DocumentSpan::collect_into_goto_definition_response(
          implementations_with_modules
            .iter()
            .map(|(i, m)| (&i.document_span, m.as_ref())),
          snapshot,
          token,
        )
        .map_err(|err| {
          anyhow!("Unable to convert implementation info: {:#}", err)
        })
      }
      Self::Go(ts_server) => {
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
      Self::Js(ts_server) => {
        let outlining_spans = ts_server
          .get_outlining_spans(snapshot.clone(), module, token)
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
      Self::Go(ts_server) => {
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyIncomingCall>>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let mut incoming_calls_with_modules = IndexMap::new();
        for module in snapshot
          .document_modules
          .get_or_temp_modules_by_compiler_options_key(document)
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
        let incoming_calls = incoming_calls_with_modules
          .iter()
          .flat_map(|(c, module)| {
            if token.is_cancelled() {
              return Some(Err(anyhow!("request cancelled")));
            }
            Some(Ok(
              c.try_resolve_call_hierarchy_incoming_call(module, snapshot)?,
            ))
          })
          .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(incoming_calls))
      }
      Self::Go(ts_server) => {
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyOutgoingCall>>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let outgoing_calls = ts_server
          .provide_call_hierarchy_outgoing_calls(
            snapshot.clone(),
            module,
            module.line_index.offset_tsc(item.selection_range.start)?,
            token,
          )
          .await?;
        let outgoing_calls = outgoing_calls
          .iter()
          .flat_map(|c| {
            if token.is_cancelled() {
              return Some(Err(anyhow!("request cancelled")));
            }
            Some(Ok(
              c.try_resolve_call_hierarchy_outgoing_call(module, snapshot)?,
            ))
          })
          .collect::<Result<_, _>>()?;
        Ok(Some(outgoing_calls))
      }
      Self::Go(ts_server) => {
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyItem>>, AnyError> {
    match self {
      Self::Js(ts_server) => {
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
            let items = items
              .into_vec()
              .into_iter()
              .flat_map(|item| {
                if token.is_cancelled() {
                  return Some(Err(anyhow!("request cancelled")));
                }
                let item =
                  item.try_resolve_call_hierarchy_item(module, snapshot)?;
                Some(Ok(item))
              })
              .collect::<Result<Vec<_>, _>>()?;
            Ok(items)
          })
          .transpose()
      }
      Self::Go(ts_server) => {
        ts_server
          .provide_prepare_call_hierarchy(module, position, snapshot, token)
          .await
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn provide_rename(
    &self,
    document: &Document,
    module: &DocumentModule,
    position: lsp::Position,
    new_name: &str,
    language_server: &language_server::Inner,
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let mut locations_with_modules = IndexMap::new();
        for module in snapshot
          .document_modules
          .get_or_temp_modules_by_compiler_options_key(document)
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
      Self::Go(ts_server) => {
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
      Self::Js(ts_server) => {
        let mut selection_ranges = Vec::with_capacity(positions.len());
        for &position in positions {
          if token.is_cancelled() {
            return Err(anyhow!("request cancelled"));
          }
          let selection_range = ts_server
            .get_smart_selection_range(
              snapshot.clone(),
              module,
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
      Self::Go(ts_server) => {
        ts_server
          .provide_selection_ranges(module, positions, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_semantic_tokens_full(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::SemanticTokensResult>, AnyError> {
    let semantic_tokens = module
      .semantic_tokens_full
      .get_or_try_init(async || {
        match self {
          Self::Js(ts_server) => ts_server
            .get_encoded_semantic_classifications(
              snapshot,
              module,
              0..module.line_index.text_content_length_utf16().into(),
              token,
            )
            .await?
            .to_semantic_tokens(module.line_index.clone(), token),
          // TODO(nayeemrmn): Fix when tsgo supports semantic tokens.
          Self::Go(_) => Ok(Default::default()),
        }
      })
      .await?
      .clone();
    if semantic_tokens.data.is_empty() {
      Ok(None)
    } else {
      Ok(Some(lsp::SemanticTokensResult::Tokens(semantic_tokens)))
    }
  }

  pub async fn provide_semantic_tokens_range(
    &self,
    module: &DocumentModule,
    range: lsp::Range,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::SemanticTokensRangeResult>, AnyError> {
    if let Some(tokens) = module.semantic_tokens_full.get() {
      let tokens = super::semantic_tokens::tokens_within_range(tokens, range);
      let result = if !tokens.data.is_empty() {
        Some(lsp::SemanticTokensRangeResult::Tokens(tokens))
      } else {
        None
      };
      return Ok(result);
    }
    let semantic_tokens = match self {
      Self::Js(ts_server) => ts_server
        .get_encoded_semantic_classifications(
          snapshot,
          module,
          module.line_index.offset_tsc(range.start)?
            ..module.line_index.offset_tsc(range.end)?,
          token,
        )
        .await?
        .to_semantic_tokens(module.line_index.clone(), token)?,
      // TODO(nayeemrmn): Fix when tsgo supports semantic tokens.
      Self::Go(_) => Default::default(),
    };
    if semantic_tokens.data.is_empty() {
      Ok(None)
    } else {
      Ok(Some(lsp::SemanticTokensRangeResult::Tokens(
        semantic_tokens,
      )))
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
      Self::Js(ts_server) => {
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
            signature_help.into_signature_help(module, &snapshot, token)
          })
          .transpose()
          .map_err(|err| {
            anyhow!("Unable to convert signature help items: {:#}", err)
          })
      }
      Self::Go(ts_server) => {
        ts_server
          .provide_signature_help(module, position, context, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_will_rename_files(
    &self,
    file_renames: &[lsp::FileRename],
    language_server: &language_server::Inner,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let mut changes_with_modules = IndexMap::new();
        for rename in file_renames {
          let Some(document) = snapshot
            .document_modules
            .documents
            .get(&Uri::from_str(&rename.old_uri).unwrap())
          else {
            continue;
          };
          for module in snapshot
            .document_modules
            .get_or_temp_modules_by_compiler_options_key(&document)
            .into_values()
          {
            if token.is_cancelled() {
              return Err(anyhow!("request cancelled"));
            }
            let options = snapshot
              .config
              .language_settings_for_specifier(&module.specifier)
              .map(|s| s.update_imports_on_file_move.clone())
              .unwrap_or_default();
            // Note that `Always` and `Prompt` are treated the same in the server, the
            // client will worry about that after receiving the edits.
            if options.enabled == UpdateImportsOnFileMoveEnabled::Never {
              continue;
            }
            let changes = ts_server
              .get_edits_for_file_rename(
                snapshot.clone(),
                &module,
                &uri_to_url(&Uri::from_str(&rename.new_uri).unwrap()),
                token,
              )
              .await?;
            let changes = fix_ts_import_changes_for_file_rename(
              changes,
              &rename.new_uri,
              &module,
              language_server,
              token,
            )
            .map_err(|err| {
              anyhow!("Unable to fix import changes: {:#}", err)
            })?;
            if !changes.is_empty() {
              changes_with_modules
                .extend(changes.into_iter().map(|c| (c, module.clone())));
            }
          }
        }
        file_text_changes_to_workspace_edit(
          changes_with_modules.iter().map(|(c, m)| (c, m.as_ref())),
          &snapshot,
          token,
        )
      }
      // TODO(nayeemrmn): Fix when tsgo supports edits for file renames.
      Self::Go(_) => Ok(None),
    }
  }

  pub async fn provide_inlay_hint(
    &self,
    module: &DocumentModule,
    range: lsp::Range,
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::InlayHint>>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let text_span =
          tsc::TextSpan::from_range(range, module.line_index.clone()).map_err(
            |err| {
              anyhow!("Failed to convert range to tsc text span: {:#}", err)
            },
          )?;
        let mut inlay_hints = ts_server
          .provide_inlay_hints(snapshot.clone(), module, text_span, token)
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
                Ok(inlay_hint.to_lsp(module, snapshot))
              })
              .collect()
          })
          .transpose()
      }
      Self::Go(ts_server) => {
        ts_server
          .provide_inlay_hint(module, range, snapshot, token)
          .await
      }
    }
  }

  pub async fn provide_workspace_symbol(
    &self,
    query: &str,
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SymbolInformation>>, AnyError> {
    match self {
      Self::Js(ts_server) => {
        let mut items_with_scopes = IndexMap::new();
        for (compiler_options_key, compiler_options_data) in
          snapshot.compiler_options_resolver.entries()
        {
          let scope = compiler_options_data
            .workspace_dir_or_source_url
            .as_ref()
            .and_then(|s| snapshot.config.tree.scope_for_specifier(s));
          if token.is_cancelled() {
            return Err(anyhow!("request cancelled"));
          }
          let items = ts_server
          .get_navigate_to_items(
            snapshot.clone(),
            query.to_string(),
            // this matches vscode's hard coded result count
            Some(256),
            compiler_options_key,
            scope,
            // TODO(nayeemrmn): Support notebook scopes here.
            None,
            token,
          )
          .await
          .inspect_err(|err| {
            lsp_warn!(
              "Unable to get signature help items from TypeScript: {:#}\nScope: {}",
              err,
              scope.map(|s| s.as_str()).unwrap_or("null"),
            );
          })
          .unwrap_or_default();
          items_with_scopes.extend(
            items
              .into_iter()
              .map(|i| (i, (scope, compiler_options_key))),
          );
        }
        let symbol_information = items_with_scopes
          .into_iter()
          .flat_map(|(item, (scope, compiler_options_key))| {
            if token.is_cancelled() {
              return Some(Err(anyhow!("request cancelled")));
            }
            Some(Ok(item.to_symbol_information(
              scope.map(|s| s.as_ref()),
              compiler_options_key,
              snapshot,
            )?))
          })
          .collect::<Result<Vec<_>, _>>()?;
        let symbol_information = if symbol_information.is_empty() {
          None
        } else {
          Some(symbol_information)
        };
        Ok(symbol_information)
      }
      Self::Go(ts_server) => {
        ts_server
          .provide_workspace_symbol(query, snapshot, token)
          .await
      }
    }
  }
}
