// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use deno_ast::diagnostics::Diagnostic;
use deno_ast::diagnostics::DiagnosticLevel;
use deno_ast::diagnostics::DiagnosticLocation;
use deno_ast::diagnostics::DiagnosticSnippet;
use deno_ast::diagnostics::DiagnosticSnippetHighlight;
use deno_ast::diagnostics::DiagnosticSnippetHighlightStyle;
use deno_ast::diagnostics::DiagnosticSourcePos;
use deno_ast::diagnostics::DiagnosticSourceRange;
use deno_ast::swc::common::util::take::Take;
use deno_ast::SourcePos;
use deno_ast::SourceRanged;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_graph::FastCheckDiagnostic;
use lsp_types::Url;

use super::unfurl::SpecifierUnfurlerDiagnostic;

#[derive(Clone, Default)]
pub struct PublishDiagnosticsCollector {
  diagnostics: Arc<Mutex<Vec<PublishDiagnostic>>>,
}

impl PublishDiagnosticsCollector {
  pub fn print_and_error(&self) -> Result<(), AnyError> {
    let mut errors = 0;
    let mut has_slow_types_errors = false;
    let mut diagnostics = self.diagnostics.lock().unwrap().take();

    diagnostics.sort_by_cached_key(|d| d.sorting_key());

    for diagnostic in diagnostics {
      eprint!("{}", diagnostic.display());
      if matches!(diagnostic.level(), DiagnosticLevel::Error) {
        errors += 1;
      }
      if matches!(diagnostic, PublishDiagnostic::FastCheck(..)) {
        has_slow_types_errors = true;
      }
    }
    if errors > 0 {
      if has_slow_types_errors {
        eprintln!(
          "This package contains errors for slow types. Fixing these errors will:\n"
        );
        eprintln!(
          "  1. Significantly improve your package users' type checking performance."
        );
        eprintln!("  2. Improve the automatic documentation generation.");
        eprintln!("  3. Enable automatic .d.ts generation for Node.js.");
        eprintln!(
          "\nDon't want to bother? You can choose to skip this step by"
        );
        eprintln!("providing the --allow-slow-types flag.\n");
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
  SpecifierUnfurl(SpecifierUnfurlerDiagnostic),
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
    text_info: SourceTextInfo,
    referrer: deno_graph::Range,
  },
  UnsupportedJsxTsx {
    specifier: Url,
  },
}

impl PublishDiagnostic {
  fn sorting_key(&self) -> (String, String, Option<SourcePos>) {
    let loc = self.location();

    let (specifier, source_pos) = match loc {
      DiagnosticLocation::Module { specifier } => (specifier.to_string(), None),
      DiagnosticLocation::Path { path } => (path.display().to_string(), None),
      DiagnosticLocation::ModulePosition {
        specifier,
        source_pos,
        text_info,
      } => (
        specifier.to_string(),
        Some(match source_pos {
          DiagnosticSourcePos::SourcePos(s) => s,
          DiagnosticSourcePos::ByteIndex(index) => {
            text_info.range().start() + index
          }
          DiagnosticSourcePos::LineAndCol { line, column } => {
            text_info.line_start(line) + column
          }
        }),
      ),
    };

    (self.code().to_string(), specifier, source_pos)
  }
}

impl Diagnostic for PublishDiagnostic {
  fn level(&self) -> DiagnosticLevel {
    use PublishDiagnostic::*;
    match self {
      FastCheck(FastCheckDiagnostic::UnsupportedJavaScriptEntrypoint {
        ..
      }) => DiagnosticLevel::Warning,
      FastCheck(_) => DiagnosticLevel::Error,
      SpecifierUnfurl(_) => DiagnosticLevel::Warning,
      InvalidPath { .. } => DiagnosticLevel::Error,
      DuplicatePath { .. } => DiagnosticLevel::Error,
      UnsupportedFileType { .. } => DiagnosticLevel::Warning,
      InvalidExternalImport { .. } => DiagnosticLevel::Error,
      UnsupportedJsxTsx { .. } => DiagnosticLevel::Warning,
    }
  }

  fn code(&self) -> Cow<'_, str> {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => diagnostic.code(),
      SpecifierUnfurl(diagnostic) => Cow::Borrowed(diagnostic.code()),
      InvalidPath { .. } => Cow::Borrowed("invalid-path"),
      DuplicatePath { .. } => Cow::Borrowed("case-insensitive-duplicate-path"),
      UnsupportedFileType { .. } => Cow::Borrowed("unsupported-file-type"),
      InvalidExternalImport { .. } => Cow::Borrowed("invalid-external-import"),
      UnsupportedJsxTsx { .. } => Cow::Borrowed("unsupported-jsx-tsx"),
    }
  }

  fn message(&self) -> Cow<'_, str> {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => diagnostic.message(),
      SpecifierUnfurl(diagnostic) => Cow::Borrowed(diagnostic.message()),
      InvalidPath { message, .. } => Cow::Borrowed(message.as_str()),
      DuplicatePath { .. } => {
        Cow::Borrowed("package path is a case insensitive duplicate of another path in the package")
      }
      UnsupportedFileType { kind, .. } => {
        Cow::Owned(format!("unsupported file type '{kind}'"))
      }
      InvalidExternalImport { kind, .. } => Cow::Owned(format!("invalid import to a {kind} specifier")),
      UnsupportedJsxTsx { .. } => Cow::Borrowed("JSX and TSX files are currently not supported"),
    }
  }

  fn location(&self) -> DiagnosticLocation {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => diagnostic.location(),
      SpecifierUnfurl(diagnostic) => match diagnostic {
        SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport {
          specifier,
          text_info,
          range,
        } => DiagnosticLocation::ModulePosition {
          specifier: Cow::Borrowed(specifier),
          text_info: Cow::Borrowed(text_info),
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
      InvalidExternalImport {
        referrer,
        text_info,
        ..
      } => DiagnosticLocation::ModulePosition {
        specifier: Cow::Borrowed(&referrer.specifier),
        text_info: Cow::Borrowed(text_info),
        source_pos: DiagnosticSourcePos::LineAndCol {
          line: referrer.start.line,
          column: referrer.start.character,
        },
      },
      UnsupportedJsxTsx { specifier } => DiagnosticLocation::Module {
        specifier: Cow::Borrowed(specifier),
      },
    }
  }

  fn snippet(&self) -> Option<DiagnosticSnippet<'_>> {
    match &self {
      PublishDiagnostic::FastCheck(diagnostic) => diagnostic.snippet(),
      PublishDiagnostic::SpecifierUnfurl(diagnostic) => match diagnostic {
        SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport {
          text_info,
          range,
          ..
        } => Some(DiagnosticSnippet {
          source: Cow::Borrowed(text_info),
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
      PublishDiagnostic::InvalidExternalImport {
        referrer,
        text_info,
        ..
      } => Some(DiagnosticSnippet {
        source: Cow::Borrowed(text_info),
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
      }),
      PublishDiagnostic::UnsupportedJsxTsx { .. } => None,
    }
  }

  fn hint(&self) -> Option<Cow<'_, str>> {
    match &self {
      PublishDiagnostic::FastCheck(diagnostic) => diagnostic.hint(),
      PublishDiagnostic::SpecifierUnfurl(_) => None,
      PublishDiagnostic::InvalidPath { .. } => Some(
        Cow::Borrowed("rename or remove the file, or add it to 'publish.exclude' in the config file"),
      ),
      PublishDiagnostic::DuplicatePath { .. } => Some(
        Cow::Borrowed("rename or remove the file"),
      ),
      PublishDiagnostic::UnsupportedFileType { .. } => Some(
        Cow::Borrowed("remove the file, or add it to 'publish.exclude' in the config file"),
      ),
      PublishDiagnostic::InvalidExternalImport { .. } => Some(Cow::Borrowed("replace this import with one from jsr or npm, or vendor the dependency into your package")),
      PublishDiagnostic::UnsupportedJsxTsx { .. } => None,
    }
  }

  fn snippet_fixed(&self) -> Option<DiagnosticSnippet<'_>> {
    match &self {
      PublishDiagnostic::InvalidExternalImport { imported, .. } => {
        match super::api::get_jsr_alternative(imported) {
          Some(replacement) => {
            let replacement = SourceTextInfo::new(replacement.into());
            let start = replacement.line_start(0);
            let end = replacement.line_end(0);
            Some(DiagnosticSnippet {
              source: Cow::Owned(replacement),
              highlight: DiagnosticSnippetHighlight {
                style: DiagnosticSnippetHighlightStyle::Hint,
                range: DiagnosticSourceRange {
                  start: DiagnosticSourcePos::SourcePos(start),
                  end: DiagnosticSourcePos::SourcePos(end),
                },
                description: Some("try this specifier".into()),
              },
            })
          }
          None => None,
        }
      }
      _ => None,
    }
  }

  fn info(&self) -> Cow<'_, [Cow<'_, str>]> {
    match &self {
      PublishDiagnostic::FastCheck(diagnostic) => {
        diagnostic.info()
      }
      PublishDiagnostic::SpecifierUnfurl(diagnostic) => match diagnostic {
        SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } => Cow::Borrowed(&[
          Cow::Borrowed("after publishing this package, imports from the local import map / package.json do not work"),
          Cow::Borrowed("dynamic imports that can not be analyzed at publish time will not be rewritten automatically"),
          Cow::Borrowed("make sure the dynamic import is resolvable at runtime without an import map / package.json")
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
      PublishDiagnostic::UnsupportedJsxTsx { .. } => Cow::Owned(vec![
        Cow::Borrowed("follow https://github.com/jsr-io/jsr/issues/24 for updates"),
      ])
    }
  }

  fn docs_url(&self) -> Option<Cow<'_, str>> {
    match &self {
      PublishDiagnostic::FastCheck(diagnostic) => diagnostic.docs_url(),
      PublishDiagnostic::SpecifierUnfurl(diagnostic) => match diagnostic {
        SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } => None,
      },
      PublishDiagnostic::InvalidPath { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/invalid-path"))
      }
      PublishDiagnostic::DuplicatePath { .. } => Some(Cow::Borrowed(
        "https://jsr.io/go/case-insensitive-duplicate-path",
      )),
      PublishDiagnostic::UnsupportedFileType { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/unsupported-file-type"))
      }
      PublishDiagnostic::InvalidExternalImport { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/invalid-external-import"))
      }
      PublishDiagnostic::UnsupportedJsxTsx { .. } => None,
    }
  }
}
