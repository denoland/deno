// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::diagnostics::Diagnostic;
use deno_ast::diagnostics::DiagnosticLevel;
use deno_ast::diagnostics::DiagnosticLocation;
use deno_ast::diagnostics::DiagnosticSnippet;
use deno_ast::diagnostics::DiagnosticSnippetHighlight;
use deno_ast::diagnostics::DiagnosticSnippetHighlightStyle;
use deno_ast::diagnostics::DiagnosticSourcePos;
use deno_ast::diagnostics::DiagnosticSourceRange;
use deno_ast::swc::common::util::take::Take;
use deno_ast::ParseDiagnostic;
use deno_ast::SourcePos;
use deno_ast::SourceRange;
use deno_ast::SourceRanged;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_graph::FastCheckDiagnostic;
use deno_semver::Version;

use super::unfurl::SpecifierUnfurlerDiagnostic;

#[derive(Clone, Default)]
pub struct PublishDiagnosticsCollector {
  diagnostics: Arc<Mutex<Vec<PublishDiagnostic>>>,
}

impl PublishDiagnosticsCollector {
  pub fn print_and_error(&self) -> Result<(), AnyError> {
    let mut errors = 0;
    let mut has_slow_types_errors = false;
    let mut diagnostics = self.diagnostics.lock().take();

    diagnostics.sort_by_cached_key(|d| d.sorting_key());

    for diagnostic in diagnostics {
      log::error!("{}", diagnostic.display());
      if matches!(diagnostic.level(), DiagnosticLevel::Error) {
        errors += 1;
      }
      if matches!(diagnostic, PublishDiagnostic::FastCheck(..)) {
        has_slow_types_errors = true;
      }
    }
    if errors > 0 {
      if has_slow_types_errors {
        log::error!(
          "This package contains errors for slow types. Fixing these errors will:\n"
        );
        log::error!(
          "  1. Significantly improve your package users' type checking performance."
        );
        log::error!("  2. Improve the automatic documentation generation.");
        log::error!("  3. Enable automatic .d.ts generation for Node.js.");
        log::error!(
          "\nDon't want to bother? You can choose to skip this step by"
        );
        log::error!("providing the --allow-slow-types flag.\n");
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

  pub fn has_error(&self) -> bool {
    self
      .diagnostics
      .lock()
      .iter()
      .any(|d| matches!(d.level(), DiagnosticLevel::Error))
  }

  pub fn push(&self, diagnostic: PublishDiagnostic) {
    self.diagnostics.lock().push(diagnostic);
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
  ExcludedModule {
    specifier: Url,
  },
  MissingConstraint {
    specifier: Url,
    specifier_text: String,
    resolved_version: Option<Version>,
    text_info: SourceTextInfo,
    referrer: deno_graph::Range,
  },
  BannedTripleSlashDirectives {
    specifier: Url,
    text_info: SourceTextInfo,
    range: SourceRange,
  },
  SyntaxError(ParseDiagnostic),
  MissingLicense {
    config_specifier: Url,
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
      ExcludedModule { .. } => DiagnosticLevel::Error,
      MissingConstraint { .. } => DiagnosticLevel::Error,
      BannedTripleSlashDirectives { .. } => DiagnosticLevel::Error,
      SyntaxError { .. } => DiagnosticLevel::Error,
      MissingLicense { .. } => DiagnosticLevel::Error,
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
      ExcludedModule { .. } => Cow::Borrowed("excluded-module"),
      MissingConstraint { .. } => Cow::Borrowed("missing-constraint"),
      BannedTripleSlashDirectives { .. } => {
        Cow::Borrowed("banned-triple-slash-directives")
      }
      SyntaxError { .. } => Cow::Borrowed("syntax-error"),
      MissingLicense { .. } => Cow::Borrowed("missing-license"),
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
      ExcludedModule { .. } => Cow::Borrowed("module in package's module graph was excluded from publishing"),
      MissingConstraint { specifier, .. } => Cow::Owned(format!("specifier '{}' is missing a version constraint", specifier)),
      BannedTripleSlashDirectives { .. } => Cow::Borrowed("triple slash directives that modify globals are not allowed"),
      SyntaxError(diagnostic) => diagnostic.message(),
      MissingLicense { .. } => Cow::Borrowed("missing license field or file"),
    }
  }

  fn location(&self) -> DiagnosticLocation {
    fn from_referrer_range<'a>(
      referrer: &'a deno_graph::Range,
      text_info: &'a SourceTextInfo,
    ) -> DiagnosticLocation<'a> {
      DiagnosticLocation::ModulePosition {
        specifier: Cow::Borrowed(&referrer.specifier),
        text_info: Cow::Borrowed(text_info),
        source_pos: DiagnosticSourcePos::LineAndCol {
          line: referrer.start.line,
          column: referrer.start.character,
        },
      }
    }

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
      } => from_referrer_range(referrer, text_info),
      UnsupportedJsxTsx { specifier } => DiagnosticLocation::Module {
        specifier: Cow::Borrowed(specifier),
      },
      ExcludedModule { specifier } => DiagnosticLocation::Module {
        specifier: Cow::Borrowed(specifier),
      },
      MissingConstraint {
        referrer,
        text_info,
        ..
      } => from_referrer_range(referrer, text_info),
      BannedTripleSlashDirectives {
        specifier,
        range,
        text_info,
      } => DiagnosticLocation::ModulePosition {
        specifier: Cow::Borrowed(specifier),
        source_pos: DiagnosticSourcePos::SourcePos(range.start),
        text_info: Cow::Borrowed(text_info),
      },
      SyntaxError(diagnostic) => diagnostic.location(),
      MissingLicense { config_specifier } => DiagnosticLocation::Module {
        specifier: Cow::Borrowed(config_specifier),
      },
    }
  }

  fn snippet(&self) -> Option<DiagnosticSnippet<'_>> {
    fn from_range<'a>(
      text_info: &'a SourceTextInfo,
      referrer: &'a deno_graph::Range,
    ) -> Option<DiagnosticSnippet<'a>> {
      if referrer.start.line == 0 && referrer.start.character == 0 {
        return None; // no range, probably a jsxImportSource import
      }

      Some(DiagnosticSnippet {
        source: Cow::Borrowed(text_info),
        highlights: vec![DiagnosticSnippetHighlight {
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
        }],
      })
    }

    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => diagnostic.snippet(),
      SpecifierUnfurl(diagnostic) => match diagnostic {
        SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport {
          text_info,
          range,
          ..
        } => Some(DiagnosticSnippet {
          source: Cow::Borrowed(text_info),
          highlights: vec![DiagnosticSnippetHighlight {
            style: DiagnosticSnippetHighlightStyle::Warning,
            range: DiagnosticSourceRange {
              start: DiagnosticSourcePos::SourcePos(range.start),
              end: DiagnosticSourcePos::SourcePos(range.end),
            },
            description: Some("the unanalyzable dynamic import".into()),
          }],
        }),
      },
      InvalidPath { .. } => None,
      DuplicatePath { .. } => None,
      UnsupportedFileType { .. } => None,
      InvalidExternalImport {
        referrer,
        text_info,
        ..
      } => from_range(text_info, referrer),
      UnsupportedJsxTsx { .. } => None,
      ExcludedModule { .. } => None,
      MissingConstraint {
        referrer,
        text_info,
        ..
      } => from_range(text_info, referrer),
      BannedTripleSlashDirectives {
        range, text_info, ..
      } => Some(DiagnosticSnippet {
        source: Cow::Borrowed(text_info),
        highlights: vec![DiagnosticSnippetHighlight {
          style: DiagnosticSnippetHighlightStyle::Error,
          range: DiagnosticSourceRange {
            start: DiagnosticSourcePos::SourcePos(range.start),
            end: DiagnosticSourcePos::SourcePos(range.end),
          },
          description: Some("the triple slash directive".into()),
        }],
      }),
      SyntaxError(diagnostic) => diagnostic.snippet(),
      MissingLicense { .. } => None,
    }
  }

  fn hint(&self) -> Option<Cow<'_, str>> {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => diagnostic.hint(),
      SpecifierUnfurl(_) => None,
      InvalidPath { .. } => Some(
        Cow::Borrowed("rename or remove the file, or add it to 'publish.exclude' in the config file"),
      ),
      DuplicatePath { .. } => Some(
        Cow::Borrowed("rename or remove the file"),
      ),
      UnsupportedFileType { .. } => Some(
        Cow::Borrowed("remove the file, or add it to 'publish.exclude' in the config file"),
      ),
      InvalidExternalImport { .. } => Some(Cow::Borrowed("replace this import with one from jsr or npm, or vendor the dependency into your package")),
      UnsupportedJsxTsx { .. } => None,
      ExcludedModule { .. } => Some(
        Cow::Borrowed("remove the module from 'exclude' and/or 'publish.exclude' in the config file or use 'publish.exclude' with a negative glob to unexclude from gitignore"),
      ),
      MissingConstraint { specifier_text, .. } => {
        Some(Cow::Borrowed(if specifier_text.starts_with("jsr:") || specifier_text.starts_with("npm:") {
          "specify a version constraint for the specifier"
        } else {
          "specify a version constraint for the specifier in the import map"
        }))
      },
      BannedTripleSlashDirectives { .. } => Some(
        Cow::Borrowed("remove the triple slash directive"),
      ),
      SyntaxError(diagnostic) => diagnostic.hint(),
      MissingLicense { .. } => Some(
        Cow::Borrowed("add a \"license\" field. Alternatively, add a LICENSE file to the package and ensure it is not ignored from being published"),
      ),
    }
  }

  fn snippet_fixed(&self) -> Option<DiagnosticSnippet<'_>> {
    use PublishDiagnostic::*;
    match &self {
      InvalidExternalImport { imported, .. } => {
        match super::api::get_jsr_alternative(imported) {
          Some(replacement) => {
            let replacement = SourceTextInfo::new(replacement.into());
            let start = replacement.line_start(0);
            let end = replacement.line_end(0);
            Some(DiagnosticSnippet {
              source: Cow::Owned(replacement),
              highlights: vec![DiagnosticSnippetHighlight {
                style: DiagnosticSnippetHighlightStyle::Hint,
                range: DiagnosticSourceRange {
                  start: DiagnosticSourcePos::SourcePos(start),
                  end: DiagnosticSourcePos::SourcePos(end),
                },
                description: Some("try this specifier".into()),
              }],
            })
          }
          None => None,
        }
      }
      SyntaxError(diagnostic) => diagnostic.snippet_fixed(),
      FastCheck(_)
      | SpecifierUnfurl(_)
      | InvalidPath { .. }
      | DuplicatePath { .. }
      | UnsupportedFileType { .. }
      | UnsupportedJsxTsx { .. }
      | ExcludedModule { .. }
      | MissingConstraint { .. }
      | BannedTripleSlashDirectives { .. }
      | MissingLicense { .. } => None,
    }
  }

  fn info(&self) -> Cow<'_, [Cow<'_, str>]> {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => {
        diagnostic.info()
      }
      SpecifierUnfurl(diagnostic) => match diagnostic {
        SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } => Cow::Borrowed(&[
          Cow::Borrowed("after publishing this package, imports from the local import map / package.json do not work"),
          Cow::Borrowed("dynamic imports that can not be analyzed at publish time will not be rewritten automatically"),
          Cow::Borrowed("make sure the dynamic import is resolvable at runtime without an import map / package.json")
        ]),
      },
      InvalidPath { .. } => Cow::Borrowed(&[
        Cow::Borrowed("to portably support all platforms, including windows, the allowed characters in package paths are limited"),
      ]),
      DuplicatePath { .. } => Cow::Borrowed(&[
        Cow::Borrowed("to support case insensitive file systems, no two package paths may differ only by case"),
      ]),
      UnsupportedFileType { .. } => Cow::Borrowed(&[
        Cow::Borrowed("only files and directories are supported"),
        Cow::Borrowed("the file was ignored and will not be published")
      ]),
      InvalidExternalImport { imported, .. } => Cow::Owned(vec![
        Cow::Owned(format!("the import was resolved to '{}'", imported)),
        Cow::Borrowed("this specifier is not allowed to be imported on jsr"),
        Cow::Borrowed("jsr only supports importing `jsr:`, `npm:`, and `data:` specifiers"),
      ]),
      UnsupportedJsxTsx { .. } => Cow::Owned(vec![
        Cow::Borrowed("follow https://github.com/jsr-io/jsr/issues/24 for updates"),
      ]),
      ExcludedModule { .. } => Cow::Owned(vec![
        Cow::Borrowed("excluded modules referenced via a package export will error at runtime due to not existing in the package"),
      ]),
      MissingConstraint { resolved_version, .. } => Cow::Owned(vec![
        Cow::Owned(format!(
          "the specifier resolved to version {} today, but will resolve to a different",
          resolved_version.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "<unresolved>".to_string())),
        ),
        Cow::Borrowed("major version if one is published in the future and potentially break"),
      ]),
      BannedTripleSlashDirectives { .. } => Cow::Borrowed(&[
        Cow::Borrowed("instead instruct the user of your package to specify these directives"),
        Cow::Borrowed("or set their 'lib' compiler option appropriately"),
      ]),
      SyntaxError(diagnostic) => diagnostic.info(),
      MissingLicense { .. } => Cow::Borrowed(&[]),
    }
  }

  fn docs_url(&self) -> Option<Cow<'_, str>> {
    use PublishDiagnostic::*;
    match &self {
      FastCheck(diagnostic) => diagnostic.docs_url(),
      SpecifierUnfurl(diagnostic) => match diagnostic {
        SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } => None,
      },
      InvalidPath { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/invalid-path"))
      }
      DuplicatePath { .. } => Some(Cow::Borrowed(
        "https://jsr.io/go/case-insensitive-duplicate-path",
      )),
      UnsupportedFileType { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/unsupported-file-type"))
      }
      InvalidExternalImport { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/invalid-external-import"))
      }
      UnsupportedJsxTsx { .. } => None,
      ExcludedModule { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/excluded-module"))
      }
      MissingConstraint { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/missing-constraint"))
      }
      BannedTripleSlashDirectives { .. } => Some(Cow::Borrowed(
        "https://jsr.io/go/banned-triple-slash-directives",
      )),
      SyntaxError(diagnostic) => diagnostic.docs_url(),
      MissingLicense { .. } => {
        Some(Cow::Borrowed("https://jsr.io/go/missing-license"))
      }
    }
  }
}
