use std::borrow::Cow;

// Copyright 2020-2021 the Deno authors. All rights reserved. MIT license.
use super::ExtendedLintRule;

use deno_ast::view as ast_view;
use deno_ast::SourceRange;
use deno_ast::SourceRanged;
use deno_lint::context::Context;
use deno_lint::diagnostic::LintFix;
use deno_lint::diagnostic::LintFixChange;
use deno_lint::rules::LintRule;

#[derive(Debug)]
pub struct RequireNodePrefix;

const CODE: &str = "require-node-prefix";
const MESSAGE: &str = "built-in Node modules require the \"node:\" specifier";
const HINT: &str = "Add \"node:\" prefix in front of the import specifier";
const FIX_DESC: &str = "Add \"node:\" prefix";
const DOCS_URL: &str =
  "https://docs.deno.com/runtime/manual/node/migrate/#node.js-built-ins";

impl ExtendedLintRule for RequireNodePrefix {
  fn supports_incremental_cache(&self) -> bool {
    true
  }

  fn help_docs_url(&self) -> std::borrow::Cow<'static, str> {
    Cow::Borrowed(DOCS_URL)
  }

  fn into_base(self: Box<Self>) -> Box<dyn deno_lint::rules::LintRule> {
    self
  }
}

impl LintRule for RequireNodePrefix {
  fn tags(&self) -> &'static [&'static str] {
    &["recommended"]
  }

  fn code(&self) -> &'static str {
    CODE
  }

  fn docs(&self) -> &'static str {
    include_str!("require_node_prefix.md")
  }

  fn lint_program_with_ast_view<'view>(
    &self,
    context: &mut Context<'view>,
    program: deno_lint::Program<'view>,
  ) {
    NodeBuiltinsSpecifierGlobalHandler.traverse(program, context);
  }
}

struct NodeBuiltinsSpecifierGlobalHandler;

impl NodeBuiltinsSpecifierGlobalHandler {
  fn add_diagnostic(&self, ctx: &mut Context, src: &str, range: SourceRange) {
    let specifier = format!(r#""node:{}""#, src);

    ctx.add_diagnostic_with_fixes(
      range,
      CODE,
      MESSAGE,
      Some(HINT.to_string()),
      vec![LintFix {
        description: FIX_DESC.into(),
        changes: vec![LintFixChange {
          new_text: specifier.into(),
          range,
        }],
      }],
    );
  }
}

impl Handler for NodeBuiltinsSpecifierGlobalHandler {
  fn import_decl(&mut self, decl: &ast_view::ImportDecl, ctx: &mut Context) {
    let src = decl.src.inner.value.as_str();
    if is_bare_node_builtin(src) {
      self.add_diagnostic(ctx, src, decl.src.range());
    }
  }

  fn call_expr(&mut self, expr: &ast_view::CallExpr, ctx: &mut Context) {
    if let ast_view::Callee::Import(_) = expr.callee {
      if let Some(src_expr) = expr.args.first() {
        if let ast_view::Expr::Lit(lit) = src_expr.expr {
          if let ast_view::Lit::Str(str_value) = lit {
            let src = str_value.inner.value.as_str();
            if is_bare_node_builtin(src) {
              self.add_diagnostic(ctx, src, lit.range());
            }
          }
        }
      }
    }
  }
}

// Should match https://nodejs.org/api/module.html#modulebuiltinmodules
fn is_bare_node_builtin(src: &str) -> bool {
  matches!(
    src,
    "assert"
      | "assert/strict"
      | "async_hooks"
      | "buffer"
      | "child_process"
      | "cluster"
      | "console"
      | "constants"
      | "crypto"
      | "dgram"
      | "diagnostics_channel"
      | "dns"
      | "dns/promises"
      | "domain"
      | "events"
      | "fs"
      | "fs/promises"
      | "http"
      | "http2"
      | "https"
      | "inspector"
      | "inspector/promises"
      | "module"
      | "net"
      | "os"
      | "path"
      | "path/posix"
      | "path/win32"
      | "perf_hooks"
      | "process"
      | "punycode"
      | "querystring"
      | "readline"
      | "readline/promises"
      | "repl"
      | "stream"
      | "stream/consumers"
      | "stream/promises"
      | "stream/web"
      | "string_decoder"
      | "sys"
      | "timers"
      | "timers/promises"
      | "tls"
      | "trace_events"
      | "tty"
      | "url"
      | "util"
      | "util/types"
      | "v8"
      | "vm"
      | "wasi"
      | "worker_threads"
      | "zlib"
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn node_specifier_valid() {
    assert_lint_ok! {
      NodeBuiltinsSpecifier,
      r#"import "node:path";"#,
      r#"import "node:fs";"#,
      r#"import "node:fs/promises";"#,

      r#"import * as fs from "node:fs";"#,
      r#"import * as fsPromises from "node:fs/promises";"#,
      r#"import fsPromises from "node:fs/promises";"#,

      r#"await import("node:fs");"#,
      r#"await import("node:fs/promises");"#,
    };
  }

  #[test]
  fn node_specifier_invalid() {
    assert_lint_err! {
      NodeBuiltinsSpecifier,
      MESSAGE,
      HINT,
      r#"import "path";"#: [
        {
          col: 7,
          fix: (FIX_DESC, r#"import "node:path";"#),
        }
      ],
      r#"import "fs";"#: [
        {
          col: 7,
          fix: (FIX_DESC, r#"import "node:fs";"#),
        }
      ],
      r#"import "fs/promises";"#: [
        {
          col: 7,
          fix: (FIX_DESC, r#"import "node:fs/promises";"#),
        }
      ],

      r#"import * as fs from "fs";"#: [
        {
          col: 20,
          fix: (FIX_DESC, r#"import * as fs from "node:fs";"#),
        }
      ],
      r#"import * as fsPromises from "fs/promises";"#: [
        {
          col: 28,
          fix: (FIX_DESC, r#"import * as fsPromises from "node:fs/promises";"#),
        }
      ],
      r#"import fsPromises from "fs/promises";"#: [
        {
          col: 23,
          fix: (FIX_DESC, r#"import fsPromises from "node:fs/promises";"#),
        }
      ],

      r#"await import("fs");"#: [
        {
          col: 13,
          fix: (FIX_DESC, r#"await import("node:fs");"#),
        }
      ],
      r#"await import("fs/promises");"#: [
        {
          col: 13,
          fix: (FIX_DESC, r#"await import("node:fs/promises");"#),
        }
      ]
    };
  }
}
