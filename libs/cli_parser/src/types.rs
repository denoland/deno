// Copyright 2018-2026 the Deno authors. MIT license.
/// Definition of a CLI command (root or subcommand).
/// All fields are `const`-constructible — no heap allocation.
#[derive(Debug, Clone, Copy)]
pub struct CommandDef {
  pub name: &'static str,
  pub about: &'static str,
  pub aliases: &'static [&'static str],
  pub args: &'static [ArgDef],
  /// Arg groups that share arguments across commands (e.g. permission args,
  /// compile args). These are flattened into `args` during parsing.
  pub arg_groups: &'static [&'static [ArgDef]],
  pub subcommands: &'static [CommandDef],
  /// When no subcommand name matches argv, use this subcommand's args.
  pub default_subcommand: Option<&'static str>,
  /// If true, once the first positional arg is consumed, all remaining
  /// args are treated as positional (no flag parsing).
  pub trailing_var_arg: bool,
  /// If true, pass all remaining args after subcommand name through
  /// without parsing (deploy/sandbox pattern).
  pub passthrough: bool,
}

impl CommandDef {
  pub const fn new(name: &'static str) -> Self {
    Self {
      name,
      about: "",
      aliases: &[],
      args: &[],
      arg_groups: &[],
      subcommands: &[],
      default_subcommand: None,
      trailing_var_arg: false,
      passthrough: false,
    }
  }

  /// Find a subcommand by name or alias.
  pub fn find_subcommand(&self, name: &str) -> Option<&CommandDef> {
    self
      .subcommands
      .iter()
      .find(|cmd| cmd.name == name || cmd.aliases.contains(&name))
  }

  /// Iterate over all args: own args + all arg group args.
  pub fn all_args(&self) -> impl Iterator<Item = &ArgDef> {
    self
      .args
      .iter()
      .chain(self.arg_groups.iter().flat_map(|g| g.iter()))
  }

  /// Find an arg by long name.
  pub fn find_arg_long(&self, long: &str) -> Option<&ArgDef> {
    self
      .all_args()
      .find(|a| a.long == Some(long) || a.long_aliases.contains(&long))
  }

  /// Find an arg by short name.
  pub fn find_arg_short(&self, short: char) -> Option<&ArgDef> {
    self
      .all_args()
      .find(|a| a.short == Some(short) || a.short_aliases.contains(&short))
  }
}

/// Definition of a single CLI argument/flag.
#[derive(Debug, Clone, Copy)]
pub struct ArgDef {
  /// Internal identifier for this arg (used to retrieve parsed values).
  pub name: &'static str,
  pub short: Option<char>,
  pub long: Option<&'static str>,
  pub short_aliases: &'static [char],
  pub long_aliases: &'static [&'static str],
  pub help: &'static str,
  pub action: ArgAction,
  pub num_args: NumArgs,
  /// If set, values like `--flag=a,b,c` are split on this char.
  pub value_delimiter: Option<char>,
  /// If true, value must be attached with `=` (e.g. `--flag=val`).
  pub require_equals: bool,
  pub required: bool,
  pub default_value: Option<&'static str>,
  pub global: bool,
  pub hidden: bool,
  /// If true, this is a positional argument (no `--`/`-` prefix).
  pub positional: bool,
  /// If true (and positional), once this positional starts consuming values,
  /// ALL remaining args (including flags like `--foo`) are collected as
  /// values for this positional. This implements per-positional
  /// `trailing_var_arg` behavior from clap.
  pub trailing: bool,
  /// Hint for shell completions.
  pub value_name: Option<&'static str>,
}

impl ArgDef {
  pub const fn new(name: &'static str) -> Self {
    Self {
      name,
      short: None,
      long: None,
      short_aliases: &[],
      long_aliases: &[],
      help: "",
      action: ArgAction::Set,
      num_args: NumArgs::Exact(1),
      value_delimiter: None,
      require_equals: false,
      required: false,
      default_value: None,
      global: false,
      hidden: false,
      positional: false,
      trailing: false,
      value_name: None,
    }
  }

  pub const fn long(mut self, long: &'static str) -> Self {
    self.long = Some(long);
    self
  }

  pub const fn short(mut self, short: char) -> Self {
    self.short = Some(short);
    self
  }

  pub const fn action(mut self, action: ArgAction) -> Self {
    self.action = action;
    self
  }

  pub const fn num_args(mut self, num_args: NumArgs) -> Self {
    self.num_args = num_args;
    self
  }

  pub const fn set_true(mut self) -> Self {
    self.action = ArgAction::SetTrue;
    self.num_args = NumArgs::Exact(0);
    self
  }

  pub const fn positional(mut self) -> Self {
    self.positional = true;
    self
  }

  pub const fn required(mut self) -> Self {
    self.required = true;
    self
  }

  pub const fn global(mut self) -> Self {
    self.global = true;
    self
  }

  pub const fn value_delimiter(mut self, delim: char) -> Self {
    self.value_delimiter = Some(delim);
    self
  }

  pub const fn require_equals(mut self) -> Self {
    self.require_equals = true;
    self
  }

  pub const fn hidden(mut self) -> Self {
    self.hidden = true;
    self
  }

  pub const fn default_value(mut self, val: &'static str) -> Self {
    self.default_value = Some(val);
    self
  }

  pub const fn help(mut self, help: &'static str) -> Self {
    self.help = help;
    self
  }

  pub const fn short_aliases(mut self, aliases: &'static [char]) -> Self {
    self.short_aliases = aliases;
    self
  }

  pub const fn long_aliases(
    mut self,
    aliases: &'static [&'static str],
  ) -> Self {
    self.long_aliases = aliases;
    self
  }

  pub const fn trailing(mut self) -> Self {
    self.trailing = true;
    self
  }

  pub const fn value_name(mut self, name: &'static str) -> Self {
    self.value_name = Some(name);
    self
  }
}

/// What the parser does when it encounters this argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArgAction {
  /// Boolean flag, sets to true when present.
  SetTrue,
  /// Takes a single value, last one wins.
  Set,
  /// Collects all occurrences into a Vec.
  Append,
  /// Counts occurrences (e.g. -vvv → 3).
  Count,
}

/// How many values an argument takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumArgs {
  /// Exactly N values.
  Exact(usize),
  /// 0 or 1 value (optional value).
  Optional,
  /// 0 or more values.
  ZeroOrMore,
  /// 1 or more values.
  OneOrMore,
}

/// Result of parsing: raw parsed data before conversion to typed Flags.
#[derive(Debug, Clone)]
pub struct ParseResult {
  /// Which subcommand was matched (None if default/root).
  pub subcommand: Option<String>,
  /// Parsed argument values, keyed by arg name.
  pub args: Vec<ParsedArg>,
  /// Remaining positional args after `--` or trailing var args.
  pub trailing: Vec<String>,
}

impl ParseResult {
  pub fn new() -> Self {
    Self {
      subcommand: None,
      args: Vec::new(),
      trailing: Vec::new(),
    }
  }

  /// Check if a boolean flag was set.
  pub fn get_bool(&self, name: &str) -> bool {
    self.args.iter().any(|a| a.name == name && a.is_present)
  }

  /// Get a single string value for an arg.
  pub fn get_one(&self, name: &str) -> Option<&str> {
    self
      .args
      .iter()
      .find(|a| a.name == name)
      .and_then(|a| a.values.first())
      .map(|s| s.as_str())
  }

  /// Get all values for an arg (for Append actions or multi-value args).
  pub fn get_many(&self, name: &str) -> Option<&[String]> {
    self
      .args
      .iter()
      .find(|a| a.name == name)
      .filter(|a| a.is_present)
      .map(|a| a.values.as_slice())
  }

  /// Check if an arg was explicitly provided on the command line.
  pub fn contains(&self, name: &str) -> bool {
    self.args.iter().any(|a| a.name == name && a.is_present)
  }

  /// Get the count for a Count action arg.
  pub fn get_count(&self, name: &str) -> usize {
    self
      .args
      .iter()
      .find(|a| a.name == name)
      .map(|a| a.count)
      .unwrap_or(0)
  }
}

impl Default for ParseResult {
  fn default() -> Self {
    Self::new()
  }
}

/// A single parsed argument with its values.
#[derive(Debug, Clone)]
pub struct ParsedArg {
  pub name: &'static str,
  pub values: Vec<String>,
  pub is_present: bool,
  pub count: usize,
}

impl ParsedArg {
  pub fn new(name: &'static str) -> Self {
    Self {
      name,
      values: Vec::new(),
      is_present: false,
      count: 0,
    }
  }
}
