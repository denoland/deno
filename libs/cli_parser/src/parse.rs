use crate::error::{CliError, CliErrorKind};
use crate::types::*;

/// Parse command-line arguments against a command definition.
///
/// This is the main entry point. It walks argv left-to-right, matches
/// subcommands and flags against the static `CommandDef`, and returns
/// a `ParseResult` with all parsed values.
pub fn parse(
    def: &CommandDef,
    args: &[String],
) -> Result<ParseResult, CliError> {
    let mut result = ParseResult::new();

    // Skip argv[0] (binary name)
    let args = if args.is_empty() { args } else { &args[1..] };

    // Determine if the first non-flag arg is a subcommand
    let sub_def = resolve_subcommand(def, args);
    if let Some(sub) = sub_def {
        result.subcommand = Some(sub.name.to_string());

        // Parse full args (parse_args will skip the subcommand token)
        parse_args(sub, def, args, Some(sub.name), &mut result)?;
    } else if let Some(default_name) = def.default_subcommand {
        // No subcommand matched — use the default subcommand's args
        // but leave result.subcommand as None so the converter knows
        // this is a bare/default invocation (not an explicit subcommand).
        if let Some(default_def) = def.find_subcommand(default_name) {
            parse_args(default_def, def, args, None, &mut result)?;
            // result.subcommand stays None — convert layer handles default
        } else {
            parse_args(def, def, args, None, &mut result)?;
        }
    } else {
        // No subcommands — parse against root command
        parse_args(def, def, args, None, &mut result)?;
    }

    Ok(result)
}

/// Determine which subcommand (if any) matches the args.
/// Returns the subcommand def and the name of the subcommand token
/// (so parse_args can skip it when it encounters it).
fn resolve_subcommand<'a>(
    def: &'a CommandDef,
    args: &[String],
) -> Option<&'a CommandDef> {
    // Walk past any global flags to find the subcommand position.
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            break;
        }

        if arg.starts_with("--") {
            // Long flag — check if it's a global flag
            let flag_name = arg.trim_start_matches('-');
            let flag_name = flag_name.split('=').next().unwrap_or(flag_name);
            if let Some(arg_def) = def.all_args().find(|a| {
                a.global && a.long == Some(flag_name)
            }) {
                i += 1;
                // Skip the value if this flag takes one
                if arg_def.num_args != NumArgs::Exact(0)
                    && arg_def.action != ArgAction::SetTrue
                    && !arg.contains('=')
                {
                    i += 1;
                }
                continue;
            }
            break;
        } else if arg.starts_with('-') && arg.len() > 1 {
            // Short flag — check if it's a global flag
            let short = arg.chars().nth(1).unwrap();
            if let Some(arg_def) = def.all_args().find(|a| a.global && a.short == Some(short)) {
                i += 1;
                if arg_def.num_args != NumArgs::Exact(0)
                    && arg_def.action != ArgAction::SetTrue
                    && arg.len() == 2
                {
                    i += 1;
                }
                continue;
            }
            break;
        } else {
            // Positional — check if it's a subcommand
            if let Some(sub) = def.find_subcommand(arg) {
                return Some(sub);
            }
            break;
        }
    }

    None
}

/// Parse arguments against a command definition.
/// `cmd_def` is the matched subcommand (or root), `root_def` is the root
/// for resolving global args. `skip_subcommand` is the subcommand name
/// token to skip when encountered as a positional arg.
fn parse_args(
    cmd_def: &CommandDef,
    root_def: &CommandDef,
    args: &[String],
    skip_subcommand: Option<&str>,
    result: &mut ParseResult,
) -> Result<(), CliError> {
    let mut i = 0;
    let mut positional_index = 0;
    let mut trailing_mode = false;
    let mut found_subcommand = skip_subcommand.is_none();
    let mut passthrough_from: Option<usize> = None;
    // Per-positional trailing: when set, ALL remaining args (including flags)
    // go into this positional's values instead of being parsed as flags.
    let mut positional_trailing_def: Option<&ArgDef> = None;

    // Collect all positional arg defs in order
    let positional_defs: Vec<&ArgDef> = cmd_def
        .all_args()
        .filter(|a| a.positional)
        .collect();

    while i < args.len() {
        let arg = &args[i];

        // Per-positional trailing mode: absorb everything into this positional
        if let Some(trail_def) = positional_trailing_def {
            if arg == "--" {
                // `--` still transitions to command-level trailing
                // so that `deno init --npm vite -- --serve` puts --serve
                // into the positional (through normal trailing → handled below)
                i += 1;
                while i < args.len() {
                    set_arg_value(result, trail_def, args[i].clone());
                    i += 1;
                }
                continue;
            }
            set_arg_value(result, trail_def, arg.clone());
            i += 1;
            continue;
        }

        // After `--`, everything is trailing
        if trailing_mode {
            result.trailing.push(arg.clone());
            i += 1;
            continue;
        }

        if arg == "--" {
            // Check if the next positional has `trailing: true` — if so,
            // args after `--` go into that positional, not result.trailing.
            if let Some(next_pos) = positional_defs.get(positional_index) {
                if next_pos.trailing {
                    positional_trailing_def = Some(next_pos);
                    i += 1;
                    continue;
                }
            }
            trailing_mode = true;
            i += 1;
            continue;
        }

        // Skip the subcommand token when we encounter it
        if !found_subcommand {
            if let Some(sub_name) = skip_subcommand {
                if !arg.starts_with('-') {
                    // Check if this is the subcommand name or an alias
                    if arg == sub_name
                        || cmd_def.aliases.iter().any(|a| *a == arg.as_str())
                    {
                        found_subcommand = true;
                        // If passthrough, collect everything after
                        if cmd_def.passthrough {
                            passthrough_from = Some(i + 1);
                            break;
                        }
                        i += 1;
                        continue;
                    }
                }
            }
        }

        if arg.starts_with("--") {
            // Long flag
            i = parse_long_flag(cmd_def, root_def, args, i, result)?;
        } else if arg.starts_with('-') && arg.len() > 1 {
            // Short flag(s)
            i = parse_short_flag(cmd_def, root_def, args, i, result)?;
        } else {
            // Positional argument
            if let Some(pos_def) = positional_defs.get(positional_index) {
                apply_value_with_delimiter(result, pos_def, arg);

                // If this positional has trailing: true, absorb everything
                // remaining into it (including flags)
                if pos_def.trailing {
                    positional_trailing_def = Some(pos_def);
                    i += 1;
                    continue;
                }

                // Move to next positional unless this one accepts multiple
                match pos_def.num_args {
                    NumArgs::ZeroOrMore | NumArgs::OneOrMore => {
                        // Stay on this positional, it absorbs more
                    }
                    _ => {
                        positional_index += 1;

                        // Check if the next positional has trailing: true.
                        // If so, enter per-positional trailing mode now.
                        if let Some(next_pos) =
                            positional_defs.get(positional_index)
                        {
                            if next_pos.trailing {
                                positional_trailing_def = Some(next_pos);
                                i += 1;
                                continue;
                            }
                        }

                        // If we just consumed the last positional and trailing_var_arg,
                        // everything remaining goes to trailing
                        if cmd_def.trailing_var_arg
                            && positional_index >= positional_defs.len()
                        {
                            i += 1;
                            while i < args.len() {
                                result.trailing.push(args[i].clone());
                                i += 1;
                            }
                            continue;
                        }
                    }
                }
            } else {
                // No more positional defs — treat as trailing or error
                if cmd_def.trailing_var_arg {
                    result.trailing.push(arg.clone());
                } else {
                    return Err(CliError::new(
                        CliErrorKind::UnexpectedPositional,
                        format!("unexpected argument '{arg}'"),
                    ));
                }
            }
            i += 1;
        }
    }

    // Handle passthrough: collect everything after the subcommand token
    if let Some(start) = passthrough_from {
        for j in start..args.len() {
            result.trailing.push(args[j].clone());
        }
    }

    Ok(())
}

/// Parse a `--long-flag` or `--long-flag=value`.
fn parse_long_flag(
    cmd_def: &CommandDef,
    root_def: &CommandDef,
    args: &[String],
    pos: usize,
    result: &mut ParseResult,
) -> Result<usize, CliError> {
    let arg = &args[pos];
    let after_dashes = &arg[2..]; // strip leading --

    // Handle --flag=value
    let (flag_name, inline_value) = match after_dashes.find('=') {
        Some(eq_pos) => (&after_dashes[..eq_pos], Some(&after_dashes[eq_pos + 1..])),
        None => (after_dashes, None),
    };

    // Look up the arg definition in the subcommand first, then root globals
    let arg_def = cmd_def
        .find_arg_long(flag_name)
        .or_else(|| {
            root_def
                .all_args()
                .find(|a| a.global && (a.long == Some(flag_name) || a.long_aliases.iter().any(|alias| *alias == flag_name)))
        })
        .ok_or_else(|| CliError::unknown_flag(arg, cmd_def))?;

    match arg_def.action {
        ArgAction::SetTrue => {
            set_arg_bool(result, arg_def);
            // If there's an inline value for a bool flag with num_args Optional,
            // consume it (e.g., --help=full)
            if let Some(val) = inline_value {
                if arg_def.num_args == NumArgs::Optional
                    || arg_def.num_args == NumArgs::ZeroOrMore
                {
                    set_arg_value(result, arg_def, val.to_string());
                }
            }
            Ok(pos + 1)
        }
        ArgAction::Count => {
            increment_arg_count(result, arg_def);
            Ok(pos + 1)
        }
        ArgAction::Set | ArgAction::Append => {
            match arg_def.num_args {
                NumArgs::Exact(0) => {
                    set_arg_bool(result, arg_def);
                    Ok(pos + 1)
                }
                NumArgs::Optional => {
                    if let Some(val) = inline_value {
                        // Track occurrence for this flag appearance
                        set_arg_bool(result, arg_def);
                        apply_value_with_delimiter(result, arg_def, val);
                    } else if arg_def.require_equals {
                        // Optional with require_equals and no `=`: present but no value
                        set_arg_bool(result, arg_def);
                    } else if pos + 1 < args.len() && !args[pos + 1].starts_with('-') {
                        set_arg_value(result, arg_def, args[pos + 1].clone());
                        return Ok(pos + 2);
                    } else {
                        set_arg_bool(result, arg_def);
                    }
                    Ok(pos + 1)
                }
                NumArgs::ZeroOrMore => {
                    set_arg_bool(result, arg_def);
                    if let Some(val) = inline_value {
                        apply_value_with_delimiter(result, arg_def, val);
                    } else if !arg_def.require_equals {
                        // Consume subsequent non-flag args
                        let mut next = pos + 1;
                        while next < args.len()
                            && !args[next].starts_with('-')
                        {
                            set_arg_value(result, arg_def, args[next].clone());
                            next += 1;
                        }
                        return Ok(next);
                    }
                    Ok(pos + 1)
                }
                NumArgs::OneOrMore | NumArgs::Exact(_) => {
                    if let Some(val) = inline_value {
                        apply_value_with_delimiter(result, arg_def, val);
                        Ok(pos + 1)
                    } else if pos + 1 < args.len() {
                        apply_value_with_delimiter(result, arg_def, &args[pos + 1]);
                        Ok(pos + 2)
                    } else {
                        Err(CliError::missing_value(arg))
                    }
                }
            }
        }
    }
}

/// Parse short flags like `-A`, `-f value`, `-fvalue`, or combined `-ABC`.
fn parse_short_flag(
    cmd_def: &CommandDef,
    root_def: &CommandDef,
    args: &[String],
    pos: usize,
    result: &mut ParseResult,
) -> Result<usize, CliError> {
    let arg = &args[pos];
    let chars: Vec<char> = arg[1..].chars().collect();

    let mut ci = 0;
    while ci < chars.len() {
        let short = chars[ci];

        let arg_def = cmd_def
            .find_arg_short(short)
            .or_else(|| {
                root_def
                    .all_args()
                    .find(|a| a.global && (a.short == Some(short) || a.short_aliases.iter().any(|c| *c == short)))
            })
            .ok_or_else(|| {
                CliError::unknown_flag(&format!("-{short}"), cmd_def)
            })?;

        match arg_def.action {
            ArgAction::SetTrue => {
                set_arg_bool(result, arg_def);
                ci += 1;
            }
            ArgAction::Count => {
                increment_arg_count(result, arg_def);
                ci += 1;
            }
            ArgAction::Set | ArgAction::Append => {
                match arg_def.num_args {
                    NumArgs::Exact(0) => {
                        set_arg_bool(result, arg_def);
                        ci += 1;
                    }
                    NumArgs::Optional | NumArgs::ZeroOrMore => {
                        set_arg_bool(result, arg_def);
                        // If there are remaining chars, check if the next
                        // char is itself a known short flag. If so, keep
                        // iterating (combined shorts). Otherwise treat
                        // remaining as value.
                        if ci + 1 < chars.len() {
                            let next_char = chars[ci + 1];
                            let next_is_flag = cmd_def
                                .find_arg_short(next_char)
                                .or_else(|| root_def.all_args().find(|a| {
                                    a.global && a.short == Some(next_char)
                                }))
                                .is_some();
                            if !next_is_flag {
                                let remaining: String =
                                    chars[ci + 1..].iter().collect();
                                apply_value_with_delimiter(
                                    result,
                                    arg_def,
                                    &remaining,
                                );
                                return Ok(pos + 1);
                            }
                        }
                        ci += 1;
                    }
                    _ => {
                        // This flag takes a value
                        if ci + 1 < chars.len() {
                            // Rest of the short flags string is the value: -fvalue
                            let value: String = chars[ci + 1..].iter().collect();
                            apply_value_with_delimiter(result, arg_def, &value);
                            return Ok(pos + 1);
                        } else if pos + 1 < args.len() {
                            // Next arg is the value
                            apply_value_with_delimiter(
                                result,
                                arg_def,
                                &args[pos + 1],
                            );
                            return Ok(pos + 2);
                        } else {
                            return Err(CliError::missing_value(&format!(
                                "-{short}"
                            )));
                        }
                    }
                }
            }
        }
    }

    Ok(pos + 1)
}

/// Apply a value, splitting by delimiter if configured.
fn apply_value_with_delimiter(
    result: &mut ParseResult,
    arg_def: &ArgDef,
    value: &str,
) {
    if let Some(delim) = arg_def.value_delimiter {
        if value.is_empty() {
            // Empty value (e.g., --flag=) — preserve it as an empty string
            set_arg_value(result, arg_def, String::new());
        } else {
            for part in value.split(delim) {
                set_arg_value(result, arg_def, part.to_string());
            }
        }
    } else {
        set_arg_value(result, arg_def, value.to_string());
    }
}

/// Set a boolean flag in the parse result.
fn set_arg_bool(result: &mut ParseResult, arg_def: &ArgDef) {
    if let Some(existing) = result.args.iter_mut().find(|a| a.name == arg_def.name) {
        existing.is_present = true;
        existing.count += 1;
    } else {
        result.args.push(ParsedArg {
            name: arg_def.name,
            values: Vec::new(),
            is_present: true,
            count: 1,
        });
    }
}

/// Add a value to an arg in the parse result.
fn set_arg_value(result: &mut ParseResult, arg_def: &ArgDef, value: String) {
    if let Some(existing) = result.args.iter_mut().find(|a| a.name == arg_def.name) {
        existing.is_present = true;
        match arg_def.action {
            ArgAction::Set => {
                // Last value wins
                existing.values = vec![value];
            }
            _ => {
                existing.values.push(value);
            }
        }
    } else {
        result.args.push(ParsedArg {
            name: arg_def.name,
            values: vec![value],
            is_present: true,
            count: 1,
        });
    }
}

/// Increment the count for a Count-action arg.
fn increment_arg_count(result: &mut ParseResult, arg_def: &ArgDef) {
    if let Some(existing) = result.args.iter_mut().find(|a| a.name == arg_def.name) {
        existing.count += 1;
        existing.is_present = true;
    } else {
        result.args.push(ParsedArg {
            name: arg_def.name,
            values: Vec::new(),
            is_present: true,
            count: 1,
        });
    }
}
