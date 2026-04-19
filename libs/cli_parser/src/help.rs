//! Help text rendering for the Deno CLI parser.
//!
//! Walks a `CommandDef` and its `ArgDef` list to produce formatted help text.

use crate::types::*;

/// Render help text for a command.
pub fn render_help(cmd: &CommandDef) -> String {
    let mut out = String::new();

    // Header
    if !cmd.about.is_empty() {
        out.push_str(cmd.about);
        out.push_str("\n\n");
    }

    // Usage line
    out.push_str(&format!("Usage: {} [OPTIONS]", cmd.name));

    let positionals: Vec<&ArgDef> = cmd.all_args().filter(|a| a.positional).collect();
    for pos in &positionals {
        let name = pos
            .value_name
            .unwrap_or(pos.name)
            .to_uppercase();
        match pos.num_args {
            NumArgs::Optional => out.push_str(&format!(" [{name}]")),
            NumArgs::ZeroOrMore => out.push_str(&format!(" [{name}]...")),
            NumArgs::OneOrMore => out.push_str(&format!(" <{name}>...")),
            NumArgs::Exact(0) => {}
            NumArgs::Exact(_) => out.push_str(&format!(" <{name}>")),
        }
    }

    if !cmd.subcommands.is_empty() {
        out.push_str(" [COMMAND]");
    }

    out.push('\n');

    // Subcommands section
    if !cmd.subcommands.is_empty() {
        out.push_str("\nCommands:\n");
        let max_name_len = cmd
            .subcommands
            .iter()
            .filter(|s| !s.name.starts_with("json_reference"))
            .map(|s| s.name.len())
            .max()
            .unwrap_or(0);

        for sub in cmd.subcommands {
            if sub.name.starts_with("json_reference") {
                continue;
            }
            out.push_str(&format!(
                "  {:<width$}  {}\n",
                sub.name,
                sub.about,
                width = max_name_len
            ));
        }
    }

    // Options section
    let flags: Vec<&ArgDef> = cmd
        .all_args()
        .filter(|a| !a.positional && !a.hidden)
        .collect();

    if !flags.is_empty() {
        out.push_str("\nOptions:\n");
        for arg in &flags {
            let mut flag_str = String::new();
            if let Some(short) = arg.short {
                flag_str.push_str(&format!("-{short}"));
                if arg.long.is_some() {
                    flag_str.push_str(", ");
                }
            } else {
                flag_str.push_str("    ");
            }
            if let Some(long) = arg.long {
                flag_str.push_str(&format!("--{long}"));
                match arg.num_args {
                    NumArgs::Exact(0) => {}
                    NumArgs::Optional => {
                        let vn = arg.value_name.unwrap_or("VALUE");
                        if arg.require_equals {
                            flag_str.push_str(&format!("[={vn}]"));
                        } else {
                            flag_str.push_str(&format!(" [{vn}]"));
                        }
                    }
                    NumArgs::ZeroOrMore => {
                        let vn = arg.value_name.unwrap_or("VALUE");
                        if arg.require_equals {
                            flag_str.push_str(&format!("[={vn}...]"));
                        } else {
                            flag_str.push_str(&format!(" [{vn}...]"));
                        }
                    }
                    _ => {
                        let vn = arg.value_name.unwrap_or("VALUE");
                        flag_str.push_str(&format!(" <{vn}>"));
                    }
                }
            }
            out.push_str(&format!("  {:<30}  {}\n", flag_str, arg.help));
        }
    }

    out
}

/// Render help for the root command when called with `deno --help`.
pub fn render_root_help(cmd: &CommandDef) -> String {
    render_help(cmd)
}

/// Find a subcommand by name in the root and render its help.
pub fn render_subcommand_help(root: &CommandDef, name: &str) -> Option<String> {
    root.find_subcommand(name).map(|sub| render_help(sub))
}
