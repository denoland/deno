// Copyright 2018-2026 the Deno authors. MIT license.
//! Shell completion script generators.
//!
//! Generate completion scripts from static `CommandDef` definitions
//! for bash, zsh, fish, and powershell.

use crate::types::*;

/// Generate a completion script for the given shell.
pub fn generate(shell: &str, cmd: &CommandDef) -> Vec<u8> {
  match shell {
    "bash" => generate_bash(cmd),
    "zsh" => generate_zsh(cmd),
    "fish" => generate_fish(cmd),
    "powershell" => generate_powershell(cmd),
    _ => Vec::new(),
  }
}

fn generate_bash(cmd: &CommandDef) -> Vec<u8> {
  let name = cmd.name;
  let mut out = String::new();

  // Collect all subcommand names
  let subcmds: Vec<&str> = cmd.subcommands.iter().map(|s| s.name).collect();
  let subcmd_list = subcmds.join(" ");

  out.push_str(&format!(
    r#"_deno() {{
    local i cur prev opts cmds
    COMPREPLY=()
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    prev="${{COMP_WORDS[COMP_CWORD-1]}}"
    cmd=""
    opts=""

    for i in ${{COMP_WORDS[@]}}
    do
        case "${{cmd}},${{i}}" in
            ",${{COMP_WORDS[0]}}")
                cmd="{name}"
                ;;
"#
  ));

  // Add subcommand detection
  for sub in cmd.subcommands {
    out.push_str(&format!(
            "            \"{name},{}\")\\n                cmd=\"{name}__{}\"\n                ;;\n",
            sub.name,
            sub.name.replace('-', "__"),
        ));
  }

  out.push_str(
    "            *)\\n                ;;\n        esac\n    done\n\n",
  );

  // Root command completions
  let root_flags: Vec<String> = cmd
    .all_args()
    .filter(|a| !a.hidden && !a.positional)
    .filter_map(|a| a.long.map(|l| format!("--{l}")))
    .collect();

  out.push_str(&format!(
    "    case \"${{cmd}}\" in\n        {name})\n            opts=\"{} {}\"\n",
    subcmd_list,
    root_flags.join(" ")
  ));
  out.push_str(
    "            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then\n",
  );
  out.push_str(
    "                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n",
  );
  out.push_str("                return 0\n            fi\n");
  out.push_str(
    "            COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n",
  );
  out.push_str("            return 0\n            ;;\n");

  // Subcommand completions
  for sub in cmd.subcommands {
    let sub_flags: Vec<String> = sub
      .all_args()
      .filter(|a| !a.hidden && !a.positional)
      .filter_map(|a| a.long.map(|l| format!("--{l}")))
      .collect();

    out.push_str(&format!(
      "        {name}__{})\n            opts=\"{}\"\n",
      sub.name.replace('-', "__"),
      sub_flags.join(" "),
    ));
    out.push_str(
      "            COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n",
    );
    out.push_str("            return 0\n            ;;\n");
  }

  out.push_str("    esac\n}\n\n");
  out.push_str(&format!(
    "complete -F _{name} -o bashdefault -o default {name}\n"
  ));

  out.into_bytes()
}

fn generate_zsh(cmd: &CommandDef) -> Vec<u8> {
  let name = cmd.name;
  let mut out = String::new();

  out.push_str(&format!("#compdef {name}\n\n"));

  // Subcommands
  out.push_str("_deno_commands() {\n    local commands; commands=(\n");
  for sub in cmd.subcommands {
    if sub.name == "help" {
      continue;
    }
    let about = sub.about.replace('\'', "'\\''");
    out.push_str(&format!("        '{}:{}'\n", sub.name, about));
  }
  out.push_str(
    "    )\n    _describe -t commands 'deno commands' commands\n}\n\n",
  );

  // Main function
  out.push_str(&format!("_{name}() {{\n"));
  out.push_str("    local line state\n\n");
  out.push_str("    _arguments -C \\\n");

  // Global flags
  for arg in cmd.all_args().filter(|a| !a.hidden && !a.positional) {
    if let Some(long) = arg.long {
      let help = arg.help.replace('\'', "'\\''");
      if let Some(short) = arg.short {
        out.push_str(&format!(
          "        '(-{short} --{long})'{{{short},--{long}}}'[{help}]' \\\n"
        ));
      } else {
        out.push_str(&format!("        '--{long}[{help}]' \\\n"));
      }
    }
  }

  out.push_str("        \":: :_deno_commands\" \\\n");
  out.push_str("        \"*::arg:->args\" \\\n");
  out.push_str("        && ret=0\n\n");

  // Subcommand handling
  out.push_str("    case $state in\n    (args)\n");
  out.push_str("        case $line[1] in\n");

  for sub in cmd.subcommands {
    if sub.name == "help" {
      continue;
    }
    out.push_str(&format!(
      "        {})\n            _arguments \\\n",
      sub.name
    ));
    for arg in sub.all_args().filter(|a| !a.hidden && !a.positional) {
      if let Some(long) = arg.long {
        let help = arg.help.replace('\'', "'\\''");
        out.push_str(&format!("                '--{long}[{help}]' \\\n"));
      }
    }
    out.push_str("                '*:file:_files'\n            ;;\n");
  }

  out.push_str("        esac\n    ;;\n    esac\n}\n\n");
  out.push_str(&format!("_{name} \"$@\"\n"));

  out.into_bytes()
}

fn generate_fish(cmd: &CommandDef) -> Vec<u8> {
  let name = cmd.name;
  let mut out = String::new();

  // Disable file completions by default
  out.push_str(&format!("complete -c {name} -e\n\n"));

  // Condition helpers
  out.push_str(&format!(
    "function __fish_{name}_no_subcommand\n    for i in (commandline -opc)\n"
  ));
  for sub in cmd.subcommands {
    out.push_str(&format!(
      "        if test $i = '{}'\n            return 1\n        end\n",
      sub.name
    ));
  }
  out.push_str("    end\n    return 0\nend\n\n");

  // Subcommand completions
  for sub in cmd.subcommands {
    if sub.name == "help" {
      continue;
    }
    out.push_str(&format!(
      "complete -c {name} -n __fish_{name}_no_subcommand -a {} -d '{}'\n",
      sub.name,
      sub.about.replace('\'', "\\'"),
    ));
  }
  out.push('\n');

  // Global flags
  for arg in cmd.all_args().filter(|a| !a.hidden && !a.positional) {
    if let Some(long) = arg.long {
      let desc = arg.help.replace('\'', "\\'");
      if let Some(short) = arg.short {
        out.push_str(&format!(
          "complete -c {name} -s {short} -l {long} -d '{desc}'\n"
        ));
      } else {
        out.push_str(&format!("complete -c {name} -l {long} -d '{desc}'\n"));
      }
    }
  }
  out.push('\n');

  // Per-subcommand flags
  for sub in cmd.subcommands {
    if sub.name == "help" {
      continue;
    }
    for arg in sub.all_args().filter(|a| !a.hidden && !a.positional) {
      if let Some(long) = arg.long {
        let desc = arg.help.replace('\'', "\\'");
        out.push_str(&format!(
                    "complete -c {name} -n '__fish_seen_subcommand_from {}' -l {long} -d '{desc}'\n",
                    sub.name
                ));
      }
    }
  }

  out.into_bytes()
}

fn generate_powershell(cmd: &CommandDef) -> Vec<u8> {
  let name = cmd.name;
  let mut out = String::new();

  out.push_str(&format!(
        r#"Register-ArgumentCompleter -Native -CommandName '{name}' -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)
    $commandElements = $commandAst.CommandElements
    $command = @(
        '{name}'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {{
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {{
                break
            }}
            $element.Value
        }}
    ) -join ';'

    $completions = @(switch ($command) {{
"#
    ));

  // Root completions
  out.push_str(&format!("        '{name}' {{\n"));
  for sub in cmd.subcommands {
    if sub.name == "help" {
      continue;
    }
    let about = sub.about.replace('\'', "''");
    out.push_str(&format!(
            "            [CompletionResult]::new('{}', '{}', [CompletionResultType]::ParameterValue, '{}')\n",
            sub.name, sub.name, about
        ));
  }
  out.push_str("        }\n");

  // Subcommand completions
  for sub in cmd.subcommands {
    if sub.name == "help" {
      continue;
    }
    out.push_str(&format!("        '{name};{}' {{\n", sub.name));
    for arg in sub.all_args().filter(|a| !a.hidden && !a.positional) {
      if let Some(long) = arg.long {
        let help = arg.help.replace('\'', "''");
        out.push_str(&format!(
                    "            [CompletionResult]::new('--{long}', '--{long}', [CompletionResultType]::ParameterName, '{help}')\n"
                ));
      }
    }
    out.push_str("        }\n");
  }

  out.push_str("    })\n\n");
  out.push_str(
        "    $completions.Where{ $_.CompletionText -like \"$wordToComplete*\" } |\n",
    );
  out.push_str("        Sort-Object -Property ListItemText\n}\n");

  out.into_bytes()
}

// ============================================================
// Dynamic completions
// ============================================================

/// The env var the registration scripts set (to the shell name) so that a
/// callback invocation of the binary is recognized as a completion request.
/// Mirrors clap_complete's env protocol.
pub const COMPLETE_ENV_VAR: &str = "COMPLETE";

/// Generate the *dynamic* completion registration script for `shell`
/// (bash/fish/zsh). Unlike [`generate`] (which bakes the full command tree into
/// a static script), this emits a small shell function that calls back into the
/// binary with `COMPLETE=<shell>` set, so completions are computed live by
/// [`try_complete`]. `bin` is the command name (e.g. `deno`); `completer` is
/// the shell-quoted path to the executable to invoke. The wire protocol
/// (`_CLAP_COMPLETE_INDEX`, `_CLAP_IFS`, `VAR=COMPLETE`) matches the scripts
/// clap_complete used to emit, so behavior is unchanged.
pub fn generate_dynamic(shell: &str, bin: &str, completer: &str) -> Vec<u8> {
  let escaped_name = bin.replace('-', "_");
  let script = match shell {
    "bash" => BASH_REGISTRATION
      .replace("NAME", &escaped_name)
      .replace("COMPLETER", completer)
      .replace("VAR", COMPLETE_ENV_VAR)
      .replace("BIN", bin),
    "zsh" => ZSH_REGISTRATION
      .replace("NAME", &escaped_name)
      .replace("COMPLETER", completer)
      .replace("VAR", COMPLETE_ENV_VAR)
      .replace("BIN", bin),
    "fish" => format!(
      "complete --keep-order --exclusive --command {bin} --arguments \
       \"({var}=fish {completer} -- (commandline --current-process --tokenize \
       --cut-at-cursor) (commandline --current-token))\"\n",
      var = COMPLETE_ENV_VAR,
    ),
    _ => String::new(),
  };
  script.into_bytes()
}

// Reproduced from clap_complete 4.5's `env::Bash::write_registration` so the
// callback protocol is identical after dropping the clap_complete dependency.
const BASH_REGISTRATION: &str = r#"
_clap_complete_NAME() {
    local IFS=$'\013'
    local _CLAP_COMPLETE_INDEX=${COMP_CWORD}
    local _CLAP_COMPLETE_COMP_TYPE=${COMP_TYPE}
    if compopt +o nospace 2> /dev/null; then
        local _CLAP_COMPLETE_SPACE=false
    else
        local _CLAP_COMPLETE_SPACE=true
    fi
    local words=("${COMP_WORDS[@]}")
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        words[COMP_CWORD]="$2"
    fi
    COMPREPLY=( $( \
        _CLAP_IFS="$IFS" \
        _CLAP_COMPLETE_INDEX="$_CLAP_COMPLETE_INDEX" \
        _CLAP_COMPLETE_COMP_TYPE="$_CLAP_COMPLETE_COMP_TYPE" \
        _CLAP_COMPLETE_SPACE="$_CLAP_COMPLETE_SPACE" \
        VAR="bash" \
        "COMPLETER" -- "${words[@]}" \
    ) )
    if [[ $? != 0 ]]; then
        unset COMPREPLY
    elif [[ $_CLAP_COMPLETE_SPACE == false ]] && [[ "${COMPREPLY-}" =~ [=/:]$ ]]; then
        compopt -o nospace
    fi
}
if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -o nospace -o bashdefault -o nosort -F _clap_complete_NAME BIN
else
    complete -o nospace -o bashdefault -F _clap_complete_NAME BIN
fi
"#;

// Reproduced from Deno's custom `ZshCompleterUnsorted` (a modified
// clap_complete zsh script that preserves candidate order via `-o nosort`).
const ZSH_REGISTRATION: &str = r#"#compdef BIN
function _clap_dynamic_completer_NAME() {
  local _CLAP_COMPLETE_INDEX=$(expr $CURRENT - 1)
  local _CLAP_IFS=$'\n'

  local completions=("${(@f)$( \
      _CLAP_IFS="$_CLAP_IFS" \
      _CLAP_COMPLETE_INDEX="$_CLAP_COMPLETE_INDEX" \
      VAR="zsh" \
      COMPLETER -- "${words[@]}" 2>/dev/null \
  )}")

  if [[ -n $completions ]]; then
      local -a dirs=()
      local -a other=()
      local completion
      for completion in $completions; do
          local value="${completion%%:*}"
          if [[ "$value" == */ ]]; then
              local dir_no_slash="${value%/}"
              if [[ "$completion" == *:* ]]; then
                  local desc="${completion#*:}"
                  dirs+=("$dir_no_slash:$desc")
              else
                  dirs+=("$dir_no_slash")
              fi
          else
              other+=("$completion")
          fi
      done
      [[ -n $dirs ]] && _describe -V 'values' dirs -o nosort -S '/' -r '/'
      [[ -n $other ]] && _describe -V 'values' other -o nosort
  fi
}

compdef _clap_dynamic_completer_NAME BIN"#;

/// A completion candidate: the value to insert, and optional help text.
pub type Candidate = (String, Option<String>);

/// Produces completion candidates for a positional argument the built-in
/// engine can't resolve on its own (e.g. `deno task <TAB>` -> task names read
/// from `deno.json`). Called with the active (sub)command and the current
/// partial word. Returning an empty vec falls back to the shell's default
/// (file) completion. This lives CLI-side so the parser crate needs no
/// config/workspace dependencies.
pub type PositionalCompleter<'a> =
  dyn Fn(&CommandDef, &str) -> Vec<Candidate> + 'a;

/// Handle dynamic shell completion. Called when `COMPLETE` env var is set.
///
/// Reads the command line from `args`, determines what to complete based on
/// cursor position, and writes completion candidates to stdout using the
/// clap_complete env protocol (`_CLAP_COMPLETE_INDEX` for the cursor position,
/// `_CLAP_IFS` for the bash output separator), so the registration scripts
/// emitted by [`generate_dynamic`] drive it unchanged.
///
/// Returns `true` if completions were handled.
#[allow(clippy::disallowed_methods, reason = "reads completion index from env")]
pub fn try_complete(
  cmd: &CommandDef,
  args: &[String],
  shell: &str,
  positional_completer: &PositionalCompleter,
) -> bool {
  let index = std::env::var("_CLAP_COMPLETE_INDEX")
    .ok()
    .and_then(|s| s.parse::<usize>().ok())
    .unwrap_or(args.len().saturating_sub(1));

  let words: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
  let current = words.get(index).copied().unwrap_or("");

  let (active_cmd, _) = find_active_command(cmd, &words);
  let candidates = get_candidates(
    active_cmd,
    cmd,
    current,
    &words,
    index,
    positional_completer,
  );

  let stdout = std::io::stdout();
  let mut out = std::io::BufWriter::new(stdout.lock());

  use std::io::Write;
  // bash consumes candidates split on `_CLAP_IFS` (the registration script sets
  // it to `\013`); zsh/fish are newline-separated with a per-shell help suffix.
  let bash_ifs = std::env::var("_CLAP_IFS").ok();
  for (i, (value, help)) in candidates.iter().enumerate() {
    match shell {
      "zsh" => {
        if let Some(h) = help {
          let _ = writeln!(out, "{}:{}", value, h);
        } else {
          let _ = writeln!(out, "{}", value);
        }
      }
      "fish" => {
        if let Some(h) = help {
          let _ = writeln!(out, "{}\t{}", value, first_line(h));
        } else {
          let _ = writeln!(out, "{}", value);
        }
      }
      _ => {
        // bash: separate values by _CLAP_IFS, no trailing separator.
        if i > 0 {
          let _ = write!(out, "{}", bash_ifs.as_deref().unwrap_or("\n"));
        }
        let _ = write!(out, "{}", value);
      }
    }
  }

  true
}

fn first_line(s: &str) -> &str {
  s.lines().next().unwrap_or_default()
}

fn find_active_command<'a>(
  root: &'a CommandDef,
  words: &[&str],
) -> (&'a CommandDef, Option<usize>) {
  for (i, word) in words.iter().enumerate().skip(1) {
    if word.starts_with('-') {
      continue;
    }
    if let Some(sub) = root.find_subcommand(word) {
      return (sub, Some(i));
    }
    break;
  }
  (root, None)
}

fn get_candidates(
  active_cmd: &CommandDef,
  root: &CommandDef,
  current: &str,
  words: &[&str],
  index: usize,
  positional_completer: &PositionalCompleter,
) -> Vec<Candidate> {
  let mut candidates = Vec::new();

  // If the previous word is a flag that takes a value, defer entirely to the
  // shell's file completion (mirrors clap emitting no candidates there).
  if index >= 2 {
    let prev = words.get(index - 1).copied().unwrap_or("");
    if prev.starts_with('-') && !prev.contains('=') {
      let flag_name = prev.trim_start_matches('-');
      let takes_value = active_cmd
        .all_args()
        .chain(root.all_args().filter(|a| a.global))
        .any(|a| {
          (a.long == Some(flag_name)
            || a.short.is_some_and(|c| c.to_string() == flag_name))
            && !matches!(a.action, ArgAction::SetTrue | ArgAction::Count)
            && a.num_args != NumArgs::Exact(0)
        });
      if takes_value {
        return candidates; // file completion
      }
    }
  }

  let want_flag = current.starts_with('-');
  let in_subcommand = words
    .iter()
    .skip(1)
    .any(|w| !w.starts_with('-') && root.find_subcommand(w).is_some());

  // Value candidates come first, matching clap's ordering: subcommand names at
  // the root, or positional values inside a subcommand (e.g. `deno task <TAB>`
  // -> task names via the CLI-provided completer). Skipped when the user is
  // clearly typing a flag (`-...`).
  if !want_flag {
    let mut values: Vec<Candidate> = if !in_subcommand {
      root
        .subcommands
        .iter()
        .filter(|s| s.name != "help" && s.name != "json_reference")
        .filter(|s| s.name.starts_with(current))
        .map(|s| {
          let help = (!s.about.is_empty()).then(|| s.about.to_string());
          (s.name.to_string(), help)
        })
        .collect()
    } else {
      positional_completer(active_cmd, current)
        .into_iter()
        .filter(|(value, _)| value.starts_with(current))
        .collect()
    };
    // clap's completion engine sorts candidates; the shells' `--keep-order` /
    // `nosort` then preserve that order, so match it (the spec `.out` files
    // encode this alphabetical ordering).
    values.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.extend(values);
  }

  // Then flags: long flags are always offered (so `deno task <TAB>` also lists
  // `--config` etc.); short flags only when the user has typed a leading `-`.
  let mut flags: Vec<Candidate> = Vec::new();
  for arg in active_cmd
    .all_args()
    .chain(root.all_args().filter(|a| a.global))
  {
    if arg.hidden || arg.positional {
      continue;
    }
    if let Some(long) = arg.long {
      let flag = format!("--{long}");
      if flag.starts_with(current) {
        let help = (!arg.help.is_empty()).then(|| arg.help.to_string());
        flags.push((flag, help));
      }
    }
    if want_flag && let Some(short) = arg.short {
      let flag = format!("-{short}");
      if flag.starts_with(current) && current.len() <= 2 {
        flags.push((flag, None));
      }
    }
  }
  flags.sort_by(|a, b| a.0.cmp(&b.0));
  candidates.extend(flags);

  candidates
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::defs::DENO_ROOT;

  fn no_positional(_: &CommandDef, _: &str) -> Vec<Candidate> {
    Vec::new()
  }

  /// Drive the engine the way `try_complete` does, but return the candidates
  /// instead of writing them to stdout.
  fn complete(
    words: &[&str],
    index: usize,
    pc: &PositionalCompleter,
  ) -> Vec<Candidate> {
    let current = words.get(index).copied().unwrap_or("");
    let (active, _) = find_active_command(&DENO_ROOT, words);
    get_candidates(active, &DENO_ROOT, current, words, index, pc)
  }

  fn values(c: &[Candidate]) -> Vec<&str> {
    c.iter().map(|(v, _)| v.as_str()).collect()
  }

  #[test]
  fn completes_subcommands_at_root() {
    let c = complete(&["deno", ""], 1, &no_positional);
    let v = values(&c);
    assert!(v.contains(&"run"), "{v:?}");
    assert!(v.contains(&"task"), "{v:?}");
    assert!(v.contains(&"test"), "{v:?}");
    // internal-only subcommands are hidden
    assert!(!v.contains(&"json_reference"), "{v:?}");
  }

  #[test]
  fn completes_subcommand_prefix() {
    let c = complete(&["deno", "ru"], 1, &no_positional);
    let v = values(&c);
    assert!(v.contains(&"run"), "{v:?}");
    assert!(!v.contains(&"task"), "{v:?}");
  }

  #[test]
  fn completes_flags_in_subcommand() {
    let c = complete(&["deno", "run", "--allow-r"], 2, &no_positional);
    let v = values(&c);
    assert!(v.iter().all(|f| f.starts_with("--allow-r")), "{v:?}");
    assert!(v.contains(&"--allow-read"), "{v:?}");
  }

  #[test]
  fn value_taking_flag_defers_to_file_completion() {
    // `run --config <TAB>`: --config takes a value, so no candidates (the shell
    // does file completion).
    let c = complete(&["deno", "run", "--config", ""], 3, &no_positional);
    assert!(c.is_empty(), "{:?}", values(&c));
  }

  #[test]
  fn positional_completer_supplies_task_names_then_flags() {
    fn tasks(cmd: &CommandDef, _: &str) -> Vec<Candidate> {
      assert_eq!(cmd.name, "task");
      vec![
        ("build".to_string(), Some("Build".to_string())),
        ("test".to_string(), None),
      ]
    }
    let c = complete(&["deno", "task", ""], 2, &tasks);
    let v = values(&c);
    // task names come first...
    assert_eq!(&v[0..2], &["build", "test"], "{v:?}");
    // ...then the subcommand's long flags (e.g. --config) are also offered.
    assert!(v.iter().any(|f| f.starts_with("--")), "{v:?}");
  }

  #[test]
  fn generate_dynamic_bash_uses_callback_protocol() {
    let s =
      String::from_utf8(generate_dynamic("bash", "deno", "/bin/deno")).unwrap();
    assert!(s.contains("_clap_complete_deno"), "{s}");
    assert!(s.contains("_CLAP_COMPLETE_INDEX"), "{s}");
    assert!(s.contains("/bin/deno"), "{s}");
    assert!(s.contains("-F _clap_complete_deno deno"), "{s}");
  }

  #[test]
  fn generate_dynamic_fish_is_exclusive_keep_order() {
    let s =
      String::from_utf8(generate_dynamic("fish", "deno", "/bin/deno")).unwrap();
    assert!(s.contains("complete --keep-order --exclusive"), "{s}");
    assert!(s.contains("COMPLETE=fish"), "{s}");
  }

  #[test]
  fn generate_dynamic_zsh_preserves_order() {
    let s =
      String::from_utf8(generate_dynamic("zsh", "deno", "/bin/deno")).unwrap();
    assert!(s.contains("#compdef deno"), "{s}");
    assert!(s.contains("_clap_dynamic_completer_deno"), "{s}");
    assert!(s.contains("-o nosort"), "{s}");
  }
}
