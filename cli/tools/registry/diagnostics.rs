// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use deno_ast::swc::common::util::take::Take;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_graph::FastCheckDiagnostic;
use lsp_types::Url;

use crate::cache::LazyGraphSourceParser;
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
use crate::util::import_map::ImportMapUnfurlDiagnostic;

#[derive(Clone, Default)]
pub struct PublishDiagnosticsCollector {
  diagnostics: Arc<Mutex<Vec<PublishDiagnostic>>>,
}

impl PublishDiagnosticsCollector {
  pub fn print_and_error(
    &self,
    sources: LazyGraphSourceParser,
  ) -> Result<(), AnyError> {
    let mut errors = 0;
    let mut has_zap_errors = false;
    let diagnostics = self.diagnostics.lock().unwrap().take();
    let sources = SourceTextParsedSourceStore(sources);
    for diagnostic in diagnostics {
      eprint!("{}", diagnostic.display(&sources));
      if matches!(diagnostic.level(), DiagnosticLevel::Error) {
        errors += 1;
      }
      if matches!(diagnostic, PublishDiagnostic::FastCheck(..)) {
        has_zap_errors = true;
      }
    }
    if errors > 0 {
      if has_zap_errors {
        eprintln!(
          "This package contains Zap errors. Although conforming to Zap will"
        );
        eprintln!("significantly improve the type checking performance of your library,");
        eprintln!("you can choose to skip it by providing the --no-zap flag.");
        eprintln!();
      }

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
  FastCheck(FastCheckDiagnostic),
  ImportMapUnfurl(ImportMapUnfurlDiagnostic),
  InvalidPath {
    path: PathBuf,
    message: String,
  },
  DuplicatePath {
    path: PathBuf,
  },
  UnsupportedFileType {
    specifier: Url,
    kind: String,
  },
  InvalidExternalImport {
    kind: String,
    imported: Url,
    referrer: deno_graph::Range,
  },
}

impl Diagnostic for PublishDiagnostic {
  fn level(&self) -> DiagnosticLevel {
    use PublishDiagnostic::*;
    match self {
      FastCheck(FastCheckDiagnostic::UnsupportedJavaScriptEntrypoint {
        ..
      }) => DiagnosticLevel::Warning,
      FastCheck(_) => DiagnosticLevel::Error,
      ImportMapUnfurl(_) => DiagnosticLevel::Warning,
      InvalidPath { .. } => DiagnosticLevel::Error,
      DuplicatePath { .. } => DiagnosticLevel::Error,
      UnsupportedFileType { .. } => DiagnosticLevel::Warning,
      InvalidExternalImport { .. } => DiagnosticLevel::Error,
    }
  }

  fn code(&self) -> impl Display + '_ {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => diagnostic.code(),
      ImportMapUnfurl(diagnostic) => diagnostic.code(),
      InvalidPath { .. } => "invalid-path",
      DuplicatePath { .. } => "case-insensitive-duplicate-path",
      UnsupportedFileType { .. } => "unsupported-file-type",
      InvalidExternalImport { .. } => "invalid-external-import",
    }
  }

  fn message(&self) -> impl Display + '_ {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => Cow::Owned(diagnostic.to_string()) ,
      ImportMapUnfurl(diagnostic) => Cow::Borrowed(diagnostic.message()),
      InvalidPath { message, .. } => Cow::Borrowed(message.as_str()),
      DuplicatePath { .. } => {
        Cow::Borrowed("package path is a case insensitive duplicate of another path in the package")
      }
      UnsupportedFileType { kind, .. } => {
        Cow::Owned(format!("unsupported file type '{kind}'"))
      }
      InvalidExternalImport { kind, .. } => Cow::Owned(format!("invalid import to a {kind} specifier")),
    }
  }

  fn location(&self) -> DiagnosticLocation {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => match diagnostic.range() {
        Some(range) => DiagnosticLocation::ModulePosition {
          specifier: Cow::Borrowed(diagnostic.specifier()),
          source_pos: DiagnosticSourcePos::SourcePos(range.range.start),
        },
        None => DiagnosticLocation::Module {
          specifier: Cow::Borrowed(diagnostic.specifier()),
        },
      },
      ImportMapUnfurl(diagnostic) => match diagnostic {
        ImportMapUnfurlDiagnostic::UnanalyzableDynamicImport {
          specifier,
          range,
        } => DiagnosticLocation::ModulePosition {
          specifier: Cow::Borrowed(specifier),
          source_pos: DiagnosticSourcePos::SourcePos(range.start),
        },
      },
      InvalidPath { path, .. } => {
        DiagnosticLocation::Path { path: path.clone() }
      }
      DuplicatePath { path, .. } => {
        DiagnosticLocation::Path { path: path.clone() }
      }
      UnsupportedFileType { specifier, .. } => DiagnosticLocation::Module {
        specifier: Cow::Borrowed(specifier),
      },
      InvalidExternalImport { referrer, .. } => {
        DiagnosticLocation::ModulePosition {
          specifier: Cow::Borrowed(&referrer.specifier),
          source_pos: DiagnosticSourcePos::LineAndCol {
            line: referrer.start.line,
            column: referrer.start.character,
          },
        }
      }
    }
  }

  fn snippet(&self) -> Option<DiagnosticSnippet<'_>> {
    match &self {
      PublishDiagnostic::FastCheck(diagnostic) => {
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
      PublishDiagnostic::ImportMapUnfurl(diagnostic) => match diagnostic {
        ImportMapUnfurlDiagnostic::UnanalyzableDynamicImport {
          specifier,
          range,
        } => Some(DiagnosticSnippet {
          source: DiagnosticSnippetSource::Specifier(Cow::Borrowed(specifier)),
          highlight: DiagnosticSnippetHighlight {
            style: DiagnosticSnippetHighlightStyle::Warning,
            range: DiagnosticSourceRange {
              start: DiagnosticSourcePos::SourcePos(range.start),
              end: DiagnosticSourcePos::SourcePos(range.end),
            },
            description: Some("the unanalyzable dynamic import".into()),
          },
        }),
      },
      PublishDiagnostic::InvalidPath { .. } => None,
      PublishDiagnostic::DuplicatePath { .. } => None,
      PublishDiagnostic::UnsupportedFileType { .. } => None,
      PublishDiagnostic::InvalidExternalImport { referrer, .. } => {
        Some(DiagnosticSnippet {
          source: DiagnosticSnippetSource::Specifier(Cow::Borrowed(
            &referrer.specifier,
          )),
          highlight: DiagnosticSnippetHighlight {
            style: DiagnosticSnippetHighlightStyle::Error,
            range: DiagnosticSourceRange {
              start: DiagnosticSourcePos::LineAndCol {
                line: referrer.start.line,
                column: referrer.start.character,
              },
              end: DiagnosticSourcePos::LineAndCol {
                line: referrer.end.line,
                column: referrer.end.character,
              },
            },
            description: Some("the specifier".into()),
          },
        })
      }
    }
  }

  fn hint(&self) -> Option<impl Display + '_> {
    match &self {
      PublishDiagnostic::FastCheck(diagnostic) => Some(diagnostic.fix_hint()),
      PublishDiagnostic::ImportMapUnfurl(_) => None,
      PublishDiagnostic::InvalidPath { .. } => Some(
        "rename or remove the file, or add it to 'publish.exclude' in the config file",
      ),
      PublishDiagnostic::DuplicatePath { .. } => Some(
        "rename or remove the file",
      ),
      PublishDiagnostic::UnsupportedFileType { .. } => Some(
        "remove the file, or add it to 'publish.exclude' in the config file",
      ),
      PublishDiagnostic::InvalidExternalImport { .. } => Some("replace this import with one from jsr or npm, or vendor the dependency into your package")
    }
  }

  fn snippet_fixed(&self) -> Option<DiagnosticSnippet<'_>> {
    None
  }

  fn info(&self) -> Cow<'_, [Cow<'_, str>]> {
    match &self {
      PublishDiagnostic::FastCheck(diagnostic) => {
        let infos = diagnostic
          .additional_info()
          .iter()
          .map(|s| Cow::Borrowed(*s))
          .collect();
        Cow::Owned(infos)
      }
      PublishDiagnostic::ImportMapUnfurl(diagnostic) => match diagnostic {
        ImportMapUnfurlDiagnostic::UnanalyzableDynamicImport { .. } => Cow::Borrowed(&[
          Cow::Borrowed("after publishing this package, imports from the local import map do not work"),
          Cow::Borrowed("dynamic imports that can not be analyzed at publish time will not be rewritten automatically"),
          Cow::Borrowed("make sure the dynamic import is resolvable at runtime without an import map")
        ]),
      },
      PublishDiagnostic::InvalidPath { .. } => Cow::Borrowed(&[
        Cow::Borrowed("to portably support all platforms, including windows, the allowed characters in package paths are limited"),
      ]),
      PublishDiagnostic::DuplicatePath { .. } => Cow::Borrowed(&[
        Cow::Borrowed("to support case insensitive file systems, no two package paths may differ only by case"),
      ]),
      PublishDiagnostic::UnsupportedFileType { .. } => Cow::Borrowed(&[
        Cow::Borrowed("only files and directories are supported"),
        Cow::Borrowed("the file was ignored and will not be published")
      ]),
      PublishDiagnostic::InvalidExternalImport { imported, .. } => Cow::Owned(vec![
        Cow::Owned(format!("the import was resolved to '{}'", imported)),
        Cow::Borrowed("this specifier is not allowed to be imported on jsr"),
        Cow::Borrowed("jsr only supports importing `jsr:`, `npm:`, and `data:` specifiers"),
      ]),
    }
  }

  fn docs_url(&self) -> Option<impl Display + '_> {
    match &self {
      PublishDiagnostic::FastCheck(diagnostic) => {
        Some(format!("https://jsr.io/go/{}", diagnostic.code()))
      }
      PublishDiagnostic::ImportMapUnfurl(diagnostic) => match diagnostic {
        ImportMapUnfurlDiagnostic::UnanalyzableDynamicImport { .. } => None,
      },
      PublishDiagnostic::InvalidPath { .. } => {
        Some("https://jsr.io/go/invalid-path".to_owned())
      }
      PublishDiagnostic::DuplicatePath { .. } => {
        Some("https://jsr.io/go/case-insensitive-duplicate-path".to_owned())
      }
      PublishDiagnostic::UnsupportedFileType { .. } => {
        Some("https://jsr.io/go/unsupported-file-type".to_owned())
      }
      PublishDiagnostic::InvalidExternalImport { .. } => {
        Some("https://jsr.io/go/invalid-external-import".to_owned())
      }
    }
  }
}
