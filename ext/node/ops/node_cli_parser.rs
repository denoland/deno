// Copyright 2018-2026 the Deno authors. MIT license.

//! Node.js CLI Argument Parser - Uses node_shim crate
//!
//! This module uses the node_shim crate to parse Node.js CLI arguments
//! and translates them to Deno CLI arguments.

use deno_core::op2;
use deno_core::serde::Serialize;
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
// Re-export types from node_shim for use elsewhere in Deno
pub use node_shim::parse_args;
pub use node_shim::parse_node_options_env_var;

/// Result of translating Node.js CLI args to Deno args
#[derive(Debug, Clone, Serialize)]
#[serde(crate = "deno_core::serde")]
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
pub fn translate_to_deno_args(
  parsed_args: ParseResult,
  script_in_npm_package: bool,
) -> TranslatedArgs {
  let mut deno_args: Vec<String> = Vec::new();
  let mut node_options: Vec<String> = Vec::new();
  let needs_npm_process_state = script_in_npm_package;

  let opts = &parsed_args.options;
  let env_opts = &opts.per_isolate.per_env;

  // Handle -e/--eval or -p/--print
  if env_opts.has_eval_string {
    deno_args.push("eval".to_string());
    if env_opts.print_eval {
      deno_args.push("-p".to_string());
    }
    if !parsed_args.v8_args.is_empty() {
      deno_args.push(format!("--v8-flags={}", parsed_args.v8_args.join(",")));
    }
    deno_args.push(env_opts.eval_string.clone());
    deno_args.extend(parsed_args.remaining_args);
    return TranslatedArgs {
      deno_args,
      node_options,
      needs_npm_process_state,
    };
  }

  // Handle --v8-options flag (print V8 help and exit)
  if opts.print_v8_help {
    deno_args.push("--v8-flags=--help".to_string());
    return TranslatedArgs {
      deno_args,
      node_options,
      needs_npm_process_state,
    };
  }

  // Handle --help flag (pass through to Deno)
  if opts.print_help {
    deno_args.push("--help".to_string());
    return TranslatedArgs {
      deno_args,
      node_options,
      needs_npm_process_state,
    };
  }

  // Handle --version flag (pass through to Deno)
  if opts.print_version {
    deno_args.push("--version".to_string());
    return TranslatedArgs {
      deno_args,
      node_options,
      needs_npm_process_state,
    };
  }

  // Handle --completion-bash flag (translate to Deno completions)
  if opts.print_bash_completion {
    deno_args.push("completions".to_string());
    deno_args.push("bash".to_string());
    return TranslatedArgs {
      deno_args,
      node_options,
      needs_npm_process_state,
    };
  }

  // Handle --run flag (run package.json script via deno task)
  if !opts.run.is_empty() {
    deno_args.push("task".to_string());
    deno_args.push(opts.run.clone());
    deno_args.extend(parsed_args.remaining_args);
    return TranslatedArgs {
      deno_args,
      node_options,
      needs_npm_process_state,
    };
  }

  // Handle --test flag (run tests via deno test)
  if env_opts.test_runner {
    deno_args.push("test".to_string());
    deno_args.push("-A".to_string());

    // Add watch mode if enabled
    if env_opts.watch_mode {
      deno_args.push("--watch".to_string());
    }

    // Add V8 flags
    if !parsed_args.v8_args.is_empty() {
      deno_args.push(format!("--v8-flags={}", parsed_args.v8_args.join(",")));
    }

    deno_args.extend(parsed_args.remaining_args);
    return TranslatedArgs {
      deno_args,
      node_options,
      needs_npm_process_state,
    };
  }

  // Handle REPL (no arguments)
  if parsed_args.remaining_args.is_empty() || env_opts.force_repl {
    // Return empty args to trigger REPL behavior
    if !parsed_args.v8_args.is_empty() {
      deno_args.push(format!("--v8-flags={}", parsed_args.v8_args.join(",")));
    }
    return TranslatedArgs {
      deno_args,
      node_options,
      needs_npm_process_state,
    };
  }

  // Handle running a script
  deno_args.push("run".to_string());
  deno_args.push("-A".to_string());

  // Add watch mode if enabled
  if env_opts.watch_mode {
    if env_opts.watch_mode_paths.is_empty() {
      deno_args.push("--watch".to_string());
    } else {
      deno_args.push(format!(
        "--watch={}",
        env_opts
          .watch_mode_paths
          .iter()
          .map(|p| p.replace(',', ",,"))
          .collect::<Vec<String>>()
          .join(",")
      ));
    }
  }

  // Add env file if specified
  if env_opts.has_env_file_string {
    if env_opts.env_file.is_empty() {
      deno_args.push("--env-file".to_string());
    } else {
      deno_args.push(format!("--env-file={}", env_opts.env_file));
    }
  }

  // Add V8 flags
  if !parsed_args.v8_args.is_empty() {
    deno_args.push(format!("--v8-flags={}", parsed_args.v8_args.join(",")));
  }

  // Add conditions
  if !env_opts.conditions.is_empty() {
    for condition in &env_opts.conditions {
      deno_args.push(format!("--conditions={}", condition));
    }
  }

  // Add inspector flags
  if env_opts.debug_options.inspector_enabled {
    let arg = if env_opts.debug_options.break_first_line {
      "--inspect-brk"
    } else if env_opts.debug_options.inspect_wait {
      "--inspect-wait"
    } else {
      "--inspect"
    };
    deno_args.push(format!(
      "{}={}:{}",
      arg,
      env_opts.debug_options.host_port.host,
      env_opts.debug_options.host_port.port
    ));
  }

  // Handle --no-warnings -> --quiet
  if !env_opts.warnings {
    deno_args.push("--quiet".to_string());
    node_options.push("--no-warnings".to_string());
  }

  // Handle --pending-deprecation (pass to NODE_OPTIONS)
  if env_opts.pending_deprecation {
    node_options.push("--pending-deprecation".to_string());
  }

  // Add the script and remaining args
  deno_args.extend(parsed_args.remaining_args);

  TranslatedArgs {
    deno_args,
    node_options,
    needs_npm_process_state,
  }
}

/// Op that parses Node.js CLI arguments and translates them to Deno CLI arguments.
/// Returns an object with deno_args, node_options, and needs_npm_process_state.
/// If parsing fails, returns the original args unchanged.
#[op2]
#[serde]
pub fn op_node_translate_cli_args(
  #[serde] args: Vec<String>,
  script_in_npm_package: bool,
) -> TranslatedArgs {
  // If no args, return early with run -A
  if args.is_empty() {
    return TranslatedArgs {
      deno_args: vec!["run".to_string(), "-A".to_string()],
      node_options: vec![],
      needs_npm_process_state: script_in_npm_package,
    };
  }

  // Parse the args
  match parse_args(args.clone()) {
    Ok(parsed) => translate_to_deno_args(parsed, script_in_npm_package),
    Err(_) => {
      // If parsing fails, fall back to simple behavior:
      // just prepend "run -A" and return original args
      let mut deno_args = vec!["run".to_string(), "-A".to_string()];
      deno_args.extend(args);
      TranslatedArgs {
        deno_args,
        node_options: vec![],
        needs_npm_process_state: script_in_npm_package,
      }
    }
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

  // Tests for incompatible argument combinations
  #[test]
  fn test_check_eval_incompatible() {
    let result = parse_args(svec!["--check", "--eval", "console.log(42)"]);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
      errors
        .iter()
        .any(|e| e.contains("either --check or --eval can be used, not both"))
    );
  }

  #[test]
  fn test_test_check_incompatible() {
    let result = parse_args(svec!["--test", "--check"]);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
      errors
        .iter()
        .any(|e| e.contains("either --test or --check can be used, not both"))
    );
  }

  #[test]
  fn test_test_eval_incompatible() {
    let result = parse_args(svec!["--test", "--eval", "console.log(42)"]);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
      errors
        .iter()
        .any(|e| e.contains("either --test or --eval can be used, not both"))
    );
  }

  #[test]
  fn test_test_interactive_incompatible() {
    let result = parse_args(svec!["--test", "--interactive"]);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| {
      e.contains("either --test or --interactive can be used, not both")
    }));
  }

  #[test]
  fn test_test_watch_path_incompatible() {
    let result = parse_args(svec!["--test", "--watch-path", "."]);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| {
      e.contains("--watch-path cannot be used in combination with --test")
    }));
  }

  #[test]
  fn test_watch_check_incompatible() {
    let result = parse_args(svec!["--watch", "--check"]);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
      errors
        .iter()
        .any(|e| e.contains("either --watch or --check can be used, not both"))
    );
  }

  #[test]
  fn test_watch_eval_incompatible() {
    let result = parse_args(svec!["--watch", "--eval", "console.log(42)"]);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
      errors
        .iter()
        .any(|e| e.contains("either --watch or --eval can be used, not both"))
    );
  }

  #[test]
  fn test_watch_interactive_incompatible() {
    let result = parse_args(svec!["--watch", "--interactive"]);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| {
      e.contains("either --watch or --interactive can be used, not both")
    }));
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
    assert_eq!(result.deno_args, svec!["eval", "console.log(42)"]);
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
}
