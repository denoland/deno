// Copyright 2018-2026 the Deno authors. MIT license.

//! Node.js CLI Argument Parser - Uses node_shim crate
//!
//! This module uses the node_shim crate to parse Node.js CLI arguments
//! and translates them to Deno CLI arguments.

use deno_core::op2;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
pub enum CliParserError {
  #[error(
    "Failed to parse Node.js CLI arguments: {message}. If you believe this is a valid Node.js flag, please report it at https://github.com/denoland/deno/issues"
  )]
  ParseError { message: String },
}

pub use node_shim::DebugOptions;
pub use node_shim::EnvironmentOptions;
pub use node_shim::HostPort;
pub use node_shim::InspectPublishUid;
pub use node_shim::OptionEnvvarSettings;
pub use node_shim::OptionType;
pub use node_shim::OptionsParser;
pub use node_shim::ParseResult;
pub use node_shim::PerIsolateOptions;
pub use node_shim::PerProcessOptions;
pub use node_shim::TranslateOptions;
pub use node_shim::TranslatedArgs as NodeShimTranslatedArgs;
// Re-export types from node_shim for use elsewhere in Deno
pub use node_shim::parse_args;
pub use node_shim::parse_node_options_env_var;
pub use node_shim::translate_to_deno_args as translate_to_deno_args_impl;
pub use node_shim::wrap_eval_code;
use serde::Serialize;

/// Result of translating Node.js CLI args to Deno args
#[derive(Debug, Clone, Serialize)]
pub struct TranslatedArgs {
  /// The Deno CLI arguments
  pub deno_args: Vec<String>,
  /// Node options that should be added to NODE_OPTIONS env var
  pub node_options: Vec<String>,
  /// Whether the child process needs npm process state
  pub needs_npm_process_state: bool,
}

/// Translate parsed Node.js CLI arguments to Deno CLI arguments.
/// This is used by child_process when spawning a Deno process as Node.js.
fn translate_to_deno_args(
  parsed_args: ParseResult,
  script_in_npm_package: bool,
) -> TranslatedArgs {
  let options = TranslateOptions::for_child_process();
  let result = translate_to_deno_args_impl(parsed_args, &options);

  TranslatedArgs {
    deno_args: result.deno_args,
    node_options: result.node_options,
    needs_npm_process_state: script_in_npm_package,
  }
}

/// Op that parses Node.js CLI arguments and translates them to Deno CLI arguments.
/// Returns an object with deno_args, node_options, and needs_npm_process_state.
/// Throws an error if parsing fails - this helps identify unsupported flags
/// so they can be added to node_shim.
#[op2]
#[serde]
pub fn op_node_translate_cli_args(
  #[serde] args: Vec<String>,
  script_in_npm_package: bool,
) -> Result<TranslatedArgs, CliParserError> {
  // If no args, return early with run -A
  if args.is_empty() {
    return Ok(TranslatedArgs {
      deno_args: vec!["run".to_string(), "-A".to_string()],
      node_options: vec![],
      needs_npm_process_state: script_in_npm_package,
    });
  }

  // Parse the args
  match parse_args(args.clone()) {
    Ok(parsed) => Ok(translate_to_deno_args(parsed, script_in_npm_package)),
    Err(unknown_flags) => Err(CliParserError::ParseError {
      message: unknown_flags.join(", "),
    }),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Macro to create a Vec<String> from string literals
  macro_rules! svec {
        ($($x:expr),* $(,)?) => {
            vec![$($x.to_string()),*]
        };
    }

  #[test]
  fn test_basic_parsing() {
    let result = parse_args(svec!["--version"]).unwrap();
    assert!(result.options.print_version);
  }

  #[test]
  fn test_help_parsing() {
    let result = parse_args(svec!["--help"]).unwrap();
    assert!(result.options.print_help);
  }

  #[test]
  fn test_debug_options() {
    let result = parse_args(svec!["--inspect"]).unwrap();
    assert!(
      result
        .options
        .per_isolate
        .per_env
        .debug_options
        .inspector_enabled
    );
  }

  #[test]
  fn test_string_option() {
    let result = parse_args(svec!["--title", "myapp"]).unwrap();
    assert_eq!(result.options.title, "myapp");
  }

  #[test]
  fn test_boolean_negation() {
    let result = parse_args(svec!["--no-warnings"]).unwrap();
    assert!(!result.options.per_isolate.per_env.warnings);
  }

  #[test]
  fn test_alias_expansion() {
    let result = parse_args(svec!["-v"]).unwrap();
    assert!(result.options.print_version);
  }

  #[test]
  fn test_node_options_parsing() {
    let env_args =
      parse_node_options_env_var("--inspect --title \"my app\"").unwrap();
    assert_eq!(env_args, vec!["--inspect", "--title", "my app"]);
  }

  #[test]
  fn test_host_port_parsing() {
    let result = parse_args(svec!["--inspect-port", "127.0.0.1:9229"]).unwrap();
    assert_eq!(
      result
        .options
        .per_isolate
        .per_env
        .debug_options
        .host_port
        .host,
      "127.0.0.1"
    );
    assert_eq!(
      result
        .options
        .per_isolate
        .per_env
        .debug_options
        .host_port
        .port,
      9229
    );
  }

  #[test]
  fn test_translate_basic_script() {
    let parsed = parse_args(svec!["script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert_eq!(result.deno_args, svec!["run", "-A", "script.js"]);
    assert!(result.node_options.is_empty());
    assert!(!result.needs_npm_process_state);
  }

  #[test]
  fn test_translate_version() {
    let parsed = parse_args(svec!["--version"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert_eq!(result.deno_args, svec!["--version"]);
  }

  #[test]
  fn test_translate_help() {
    let parsed = parse_args(svec!["--help"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert_eq!(result.deno_args, svec!["--help"]);
  }

  #[test]
  fn test_translate_eval() {
    let parsed = parse_args(svec!["--eval", "console.log(42)"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    // Eval code should be wrapped for child_process
    assert!(result.deno_args.contains(&"eval".to_string()));
    // Note: deno eval has implicit permissions, so -A is not added
    // The wrapped code should contain vm.runInThisContext
    assert!(
      result
        .deno_args
        .iter()
        .any(|a| a.contains("vm.runInThisContext"))
    );
  }

  #[test]
  fn test_translate_inspect() {
    let parsed = parse_args(svec!["--inspect", "script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(
      result
        .deno_args
        .contains(&"--inspect=127.0.0.1:9229".to_string())
    );
    assert!(result.deno_args.contains(&"script.js".to_string()));
  }

  #[test]
  fn test_translate_inspect_brk() {
    let parsed = parse_args(svec!["--inspect-brk", "script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(
      result
        .deno_args
        .contains(&"--inspect-brk=127.0.0.1:9229".to_string())
    );
  }

  #[test]
  fn test_translate_watch() {
    let parsed = parse_args(svec!["--watch", "script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(result.deno_args.contains(&"--watch".to_string()));
  }

  #[test]
  fn test_translate_no_warnings() {
    let parsed = parse_args(svec!["--no-warnings", "script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(result.deno_args.contains(&"--quiet".to_string()));
    assert!(result.node_options.contains(&"--no-warnings".to_string()));
  }

  #[test]
  fn test_translate_conditions() {
    let parsed =
      parse_args(svec!["--conditions", "development", "script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(
      result
        .deno_args
        .contains(&"--conditions=development".to_string())
    );
  }

  #[test]
  fn test_translate_conditions_equals_format() {
    // Test the --conditions=custom format (with equals sign)
    let parsed = parse_args(svec!["--conditions=custom", "script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(
      result
        .deno_args
        .contains(&"--conditions=custom".to_string()),
    );
  }

  #[test]
  fn test_translate_conditions_short_alias() {
    // Test -C custom format (short alias)
    let parsed = parse_args(svec!["-C", "custom", "script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(
      result
        .deno_args
        .contains(&"--conditions=custom".to_string()),
    );
  }

  #[test]
  fn test_translate_v8_flags() {
    let parsed =
      parse_args(svec!["--max-old-space-size=4096", "script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(result.deno_args.iter().any(|a| a.contains("--v8-flags=")));
  }

  #[test]
  fn test_translate_repl() {
    let parsed = parse_args(svec![]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    // REPL should have empty deno_args (triggers Deno's REPL behavior)
    assert!(result.deno_args.is_empty());
  }

  #[test]
  fn test_translate_npm_package() {
    let parsed = parse_args(svec!["script.js"]).unwrap();
    let result = translate_to_deno_args(parsed, true);
    assert!(result.needs_npm_process_state);
  }

  #[test]
  fn test_translate_run_script() {
    let parsed = parse_args(svec!["--run", "build"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert_eq!(result.deno_args, svec!["task", "build"]);
  }

  #[test]
  fn test_translate_test_runner() {
    let parsed = parse_args(svec!["--test", "test.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(result.deno_args.contains(&"test".to_string()));
    assert!(result.deno_args.contains(&"-A".to_string()));
    assert!(result.deno_args.contains(&"test.js".to_string()));
  }

  #[test]
  fn test_translate_test_with_watch() {
    let parsed = parse_args(svec!["--test", "--watch", "test.js"]).unwrap();
    let result = translate_to_deno_args(parsed, false);
    assert!(result.deno_args.contains(&"test".to_string()));
    assert!(result.deno_args.contains(&"--watch".to_string()));
  }

  #[test]
  fn test_wrap_eval_code() {
    let wrapped = wrap_eval_code("console.log(42)");
    assert!(wrapped.contains("vm.runInThisContext"));
    assert!(wrapped.contains("process.getBuiltinModule"));
    assert!(wrapped.contains("\"console.log(42)\""));
  }
}
