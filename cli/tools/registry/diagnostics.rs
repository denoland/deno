// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::Mutex;

use deno_ast::swc::common::util::take::Take;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_graph::FastCheckDiagnostic;
use deno_graph::ParsedSourceStore;

use crate::diagnostics::Diagnostic;
use crate::diagnostics::DiagnosticLevel;
use crate::diagnostics::DiagnosticLocation;
use crate::diagnostics::DiagnosticSnippet;
use crate::diagnostics::DiagnosticSnippetHighlight;
use crate::diagnostics::DiagnosticSnippetHighlightStyle;
use crate::diagnostics::DiagnosticSnippetSource;
use crate::diagnostics::DiagnosticSourcePos;
use crate::diagnostics::DiagnosticSourceRange;
use crate::diagnostics::SourceTextParsedSourceStore;

#[derive(Clone, Default)]
pub struct PublishDiagnosticsCollector {
  diagnostics: Arc<Mutex<Vec<PublishDiagnostic>>>,
}

impl PublishDiagnosticsCollector {
  pub fn print_and_error(
    &self,
    sources: &dyn ParsedSourceStore,
  ) -> Result<(), AnyError> {
    let mut errors = 0;
    let diagnostics = self.diagnostics.lock().unwrap().take();
    let sources = SourceTextParsedSourceStore(sources);
    for diagnostic in diagnostics {
      eprintln!("{}", diagnostic.display(&sources));
      if matches!(diagnostic.level(), DiagnosticLevel::Error) {
        errors += 1;
      }
    }
    if errors > 0 {
      Err(anyhow!(
        "Found {} problem{}",
        errors,
        if errors == 1 { "" } else { "s" }
      ))
    } else {
      Ok(())
    }
  }

  pub fn push(&self, diagnostic: PublishDiagnostic) {
    self.diagnostics.lock().unwrap().push(diagnostic);
  }
}

pub enum PublishDiagnostic {
  FastCheck { diagnostic: FastCheckDiagnostic },
}

impl Diagnostic for PublishDiagnostic {
  fn level(&self) -> DiagnosticLevel {
    match self {
      PublishDiagnostic::FastCheck {
        diagnostic: FastCheckDiagnostic::UnsupportedJavaScriptEntrypoint { .. },
      } => DiagnosticLevel::Warning,
      PublishDiagnostic::FastCheck { .. } => DiagnosticLevel::Error,
    }
  }

  fn code(&self) -> impl Display + '_ {
    match &self {
      PublishDiagnostic::FastCheck { diagnostic, .. } => diagnostic.code(),
    }
  }

  fn message(&self) -> impl Display + '_ {
    match &self {
      PublishDiagnostic::FastCheck { diagnostic, .. } => diagnostic.to_string(), // todo
    }
  }

  fn location(&self) -> DiagnosticLocation {
    match &self {
      PublishDiagnostic::FastCheck { diagnostic } => match diagnostic.range() {
        Some(range) => DiagnosticLocation::PositionInFile {
          specifier: Cow::Borrowed(diagnostic.specifier()),
          source_pos: DiagnosticSourcePos::SourcePos(range.range.start),
        },
        None => DiagnosticLocation::File {
          specifier: Cow::Borrowed(diagnostic.specifier()),
        },
      },
    }
  }

  fn snippet(&self) -> Option<DiagnosticSnippet<'_>> {
    match &self {
      PublishDiagnostic::FastCheck { diagnostic } => {
        diagnostic.range().map(|range| DiagnosticSnippet {
          source: DiagnosticSnippetSource::Specifier(Cow::Borrowed(
            diagnostic.specifier(),
          )),
          highlight: DiagnosticSnippetHighlight {
            style: DiagnosticSnippetHighlightStyle::Error,
            range: DiagnosticSourceRange {
              start: DiagnosticSourcePos::SourcePos(range.range.start),
              end: DiagnosticSourcePos::SourcePos(range.range.end),
            },
            description: diagnostic.range_description().map(Cow::Borrowed),
          },
        })
      }
    }
  }

  fn hint(&self) -> Option<impl Display + '_> {
    match &self {
      PublishDiagnostic::FastCheck { diagnostic } => {
        Some(diagnostic.fix_hint())
      }
    }
  }

  fn snippet_fixed(&self) -> Option<DiagnosticSnippet<'_>> {
    None
  }

  fn info(&self) -> Cow<'_, [Cow<'_, str>]> {
    match &self {
      PublishDiagnostic::FastCheck { diagnostic } => {
        let infos = diagnostic
          .additional_info()
          .iter()
          .map(|s| Cow::Borrowed(*s))
          .collect();
        Cow::Owned(infos)
      }
    }
  }

  fn docs_url(&self) -> Option<impl Display + '_> {
    match &self {
      PublishDiagnostic::FastCheck { diagnostic } => {
        Some(format!("https://jsr.io/go/{}", diagnostic.code()))
      }
    }
  }
}
