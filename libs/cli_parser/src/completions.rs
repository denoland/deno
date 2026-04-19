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

    out.push_str("            *)\\n                ;;\n        esac\n    done\n\n");

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
    out.push_str("            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then\n");
    out.push_str("                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n");
    out.push_str("                return 0\n            fi\n");
    out.push_str("            COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n");
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
    out.push_str(&format!("complete -F _{name} -o bashdefault -o default {name}\n"));

    out.into_bytes()
}

fn generate_zsh(cmd: &CommandDef) -> Vec<u8> {
    let name = cmd.name;
    let mut out = String::new();

    out.push_str(&format!("#compdef {name}\n\n"));

    // Subcommands
    out.push_str(&format!("_deno_commands() {{\n    local commands; commands=(\n"));
    for sub in cmd.subcommands {
        if sub.name == "help" {
            continue;
        }
        let about = sub.about.replace('\'', "'\\''");
        out.push_str(&format!("        '{}:{}'\n", sub.name, about));
    }
    out.push_str("    )\n    _describe -t commands 'deno commands' commands\n}\n\n");

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
        out.push_str(&format!("        {})\n            _arguments \\\n", sub.name));
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
        out.push_str(&format!("        if test $i = '{}'\n            return 1\n        end\n", sub.name));
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
                out.push_str(&format!(
                    "complete -c {name} -l {long} -d '{desc}'\n"
                ));
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
