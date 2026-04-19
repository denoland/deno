use std::fmt;

use crate::CommandDef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliErrorKind {
    /// An unknown flag was passed.
    UnknownFlag,
    /// A required argument is missing.
    MissingRequired,
    /// A flag expected a value but didn't get one.
    MissingValue,
    /// An invalid value was provided.
    InvalidValue,
    /// --version was requested.
    DisplayVersion,
    /// --help was requested.
    DisplayHelp,
    /// Too many values for an argument.
    TooManyValues,
    /// Unexpected positional argument.
    UnexpectedPositional,
}

#[derive(Debug, Clone)]
pub struct CliError {
    pub kind: CliErrorKind,
    pub message: String,
    pub suggestion: Option<String>,
}

impl CliError {
    pub fn new(kind: CliErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn unknown_flag(
        flag: &str,
        command: &CommandDef,
    ) -> Self {
        let mut err = Self::new(
            CliErrorKind::UnknownFlag,
            format!("unexpected argument '{flag}' found"),
        );

        // Find closest match via Levenshtein distance
        if let Some(suggestion) = find_closest_flag(flag, command) {
            err.suggestion = Some(format!("a similar argument exists: '{suggestion}'"));
        }

        err
    }

    pub fn missing_value(flag: &str) -> Self {
        Self::new(
            CliErrorKind::MissingValue,
            format!("a value is required for '{flag}' but none was supplied"),
        )
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.message)?;
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n\n  tip: {suggestion}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CliError {}

/// Levenshtein distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0; b_len + 1];

    for (i, a_char) in a.chars().enumerate() {
        curr_row[0] = i + 1;
        for (j, b_char) in b.chars().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };
            curr_row[j + 1] = (prev_row[j + 1] + 1)
                .min(curr_row[j] + 1)
                .min(prev_row[j] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Find the closest matching flag name for suggestions.
fn find_closest_flag(input: &str, command: &CommandDef) -> Option<String> {
    // Strip leading dashes for comparison
    let input_stripped = input.trim_start_matches('-');

    let mut best: Option<(usize, String)> = None;

    for arg in command.all_args() {
        if let Some(long) = arg.long {
            let dist = levenshtein(input_stripped, long);
            let threshold = (long.len() / 3).max(2);
            if dist <= threshold {
                if best.as_ref().is_none_or(|(d, _)| dist < *d) {
                    best = Some((dist, format!("--{long}")));
                }
            }
        }
    }

    best.map(|(_, s)| s)
}
