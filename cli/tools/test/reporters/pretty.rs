// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use super::common;
use super::fmt::to_relative_path_or_remote_url;
use super::*;

pub struct PrettyTestReporter {
  parallel: bool,
  echo_output: bool,
  in_new_line: bool,
  phase: &'static str,
  filter: bool,
  repl: bool,
  scope_test_id: Option<usize>,
  cwd: Url,
  did_have_user_output: bool,
  started_tests: bool,
  ended_tests: bool,
  child_results_buffer:
    HashMap<usize, IndexMap<usize, (TestStepDescription, TestStepResult, u64)>>,
  summary: TestSummary,
  writer: Box<dyn std::io::Write>,
  failure_format_options: TestFailureFormatOptions,
}

impl PrettyTestReporter {
  pub fn new(
    parallel: bool,
    echo_output: bool,
    filter: bool,
    repl: bool,
    cwd: Url,
    failure_format_options: TestFailureFormatOptions,
  ) -> PrettyTestReporter {
    PrettyTestReporter {
      parallel,
      echo_output,
      in_new_line: true,
      phase: "",
      filter,
      repl,
      scope_test_id: None,
      cwd,
      did_have_user_output: false,
      started_tests: false,
      ended_tests: false,
      child_results_buffer: Default::default(),
      summary: TestSummary::new(),
      writer: Box::new(std::io::stdout()),
      failure_format_options,
    }
  }

  pub fn with_writer(self, writer: Box<dyn std::io::Write>) -> Self {
    Self { writer, ..self }
  }

  fn force_report_wait(&mut self, description: &TestDescription) {
    if !self.in_new_line {
      writeln!(&mut self.writer).ok();
    }
    if self.parallel {
      write!(
        &mut self.writer,
        "{}",
        colors::gray(format!(
          "{} => ",
          to_relative_path_or_remote_url(&self.cwd, &description.origin)
        ))
      )
      .ok();
    }
    write!(&mut self.writer, "{} ...", description.name).ok();
    self.in_new_line = false;
    // flush for faster feedback when line buffered
    std::io::stdout().flush().ok();
    self.scope_test_id = Some(description.id);
  }

  fn force_report_step_wait(&mut self, description: &TestStepDescription) {
    self.write_output_end();
    if !self.in_new_line {
      writeln!(&mut self.writer).ok();
    }
    write!(
      &mut self.writer,
      "{}{} ...",
      "  ".repeat(description.level),
      description.name
    )
    .ok();
    self.in_new_line = false;
    // flush for faster feedback when line buffered
    std::io::stdout().flush().ok();
    self.scope_test_id = Some(description.id);
  }

  fn force_report_step_result(
    &mut self,
    description: &TestStepDescription,
    result: &TestStepResult,
    elapsed: u64,
  ) {
    self.write_output_end();
    if self.in_new_line || self.scope_test_id != Some(description.id) {
      self.force_report_step_wait(description);
    }

    if !self.parallel {
      let child_results = self
        .child_results_buffer
        .remove(&description.id)
        .unwrap_or_default();
      for (desc, result, elapsed) in child_results.values() {
        self.force_report_step_result(desc, result, *elapsed);
      }
      if !child_results.is_empty() {
        self.force_report_step_wait(description);
      }
    }

    let status = match &result {
      TestStepResult::Ok => colors::green("ok").to_string(),
      TestStepResult::Ignored => colors::yellow("ignored").to_string(),
      TestStepResult::Failed(failure) => failure.format_label(),
    };
    write!(&mut self.writer, " {status}").ok();
    if let TestStepResult::Failed(failure) = result
      && let Some(inline_summary) = failure.format_inline_summary()
    {
      write!(&mut self.writer, " ({})", inline_summary).ok();
    }
    if !matches!(result, TestStepResult::Failed(TestFailure::Incomplete)) {
      write!(
        &mut self.writer,
        " {}",
        colors::gray(format!("({})", display::human_elapsed(elapsed.into())))
      )
      .ok();
    }
    writeln!(&mut self.writer).ok();
    self.in_new_line = true;
    if self.parallel {
      self.scope_test_id = None;
    } else {
      self.scope_test_id = Some(description.parent_id);
    }
    self
      .child_results_buffer
      .entry(description.parent_id)
      .or_default()
      .shift_remove(&description.id);
  }

  fn write_output_end(&mut self) {
    if self.did_have_user_output {
      writeln!(
        &mut self.writer,
        "{}",
        colors::gray(format!("----- {}output end -----", self.phase))
      )
      .ok();
      self.in_new_line = true;
      self.did_have_user_output = false;
    }
  }
}

impl TestReporter for PrettyTestReporter {
  fn report_register(&mut self, _description: &TestDescription) {}
  fn report_plan(&mut self, plan: &TestPlan) {
    self.write_output_end();
    self.summary.total += plan.total;
    self.summary.filtered_out += plan.filtered_out;
    if self.repl {
      return;
    }
    if self.parallel || (self.filter && plan.total == 0) {
      return;
    }
    let inflection = if plan.total == 1 { "test" } else { "tests" };
    writeln!(
      &mut self.writer,
      "{}",
      colors::gray(format!(
        "running {} {} from {}",
        plan.total,
        inflection,
        to_relative_path_or_remote_url(&self.cwd, &plan.origin)
      ))
    )
    .ok();
    self.in_new_line = true;
  }

  fn report_wait(&mut self, description: &TestDescription) {
    self.write_output_end();
    if !self.parallel {
      self.force_report_wait(description);
    }
    self.started_tests = true;
  }

  fn report_slow(&mut self, description: &TestDescription, elapsed: u64) {
    writeln!(
      &mut self.writer,
      "{}",
      colors::yellow_bold(format!(
        "'{}' has been running for over {}",
        description.name,
        colors::gray(format!("({})", display::human_elapsed(elapsed.into()))),
      ))
    )
    .ok();
  }
  fn report_output(&mut self, output: &[u8]) {
    if !self.echo_output {
      return;
    }

    if !self.did_have_user_output {
      self.did_have_user_output = true;
      if !self.in_new_line {
        writeln!(&mut self.writer).ok();
      }
      self.phase = if !self.started_tests {
        "pre-test "
      } else if self.ended_tests {
        "post-test "
      } else {
        ""
      };
      writeln!(
        &mut self.writer,
        "{}",
        colors::gray(format!("------- {}output -------", self.phase))
      )
      .ok();
      self.in_new_line = true;
    }

    // output everything to stdout in order to prevent
    // stdout and stderr racing
    let filtered = filter_destructive_ansi(output);
    std::io::stdout().write_all(&filtered).ok();
  }

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
  ) {
    match &result {
      TestResult::Ok => {
        self.summary.passed += 1;
      }
      TestResult::Ignored => {
        self.summary.ignored += 1;
      }
      TestResult::Failed(failure) => {
        self.summary.failed += 1;
        self
          .summary
          .failures
          .push((description.into(), failure.clone()));
      }
      TestResult::Cancelled => {
        self.summary.failed += 1;
      }
    }

    if self.parallel {
      self.force_report_wait(description);
    }

    self.write_output_end();
    if self.in_new_line || self.scope_test_id != Some(description.id) {
      self.force_report_wait(description);
    }

    let status = match result {
      TestResult::Ok => colors::green("ok").to_string(),
      TestResult::Ignored => colors::yellow("ignored").to_string(),
      TestResult::Failed(failure) => failure.format_label(),
      TestResult::Cancelled => colors::gray("cancelled").to_string(),
    };
    write!(&mut self.writer, " {status}").ok();
    if let TestResult::Failed(failure) = result
      && let Some(inline_summary) = failure.format_inline_summary()
    {
      write!(&mut self.writer, " ({})", inline_summary).ok();
    }
    writeln!(
      &mut self.writer,
      " {}",
      colors::gray(format!("({})", display::human_elapsed(elapsed.into())))
    )
    .ok();
    self.in_new_line = true;
    self.scope_test_id = None;
  }

  fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>) {
    self.summary.failed += 1;
    self
      .summary
      .uncaught_errors
      .push((origin.to_string(), error));

    if !self.in_new_line {
      writeln!(&mut self.writer).ok();
    }
    writeln!(
      &mut self.writer,
      "Uncaught error from {} {}",
      to_relative_path_or_remote_url(&self.cwd, origin),
      colors::red("FAILED")
    )
    .ok();
    self.in_new_line = true;
    self.did_have_user_output = false;
  }

  fn report_step_register(&mut self, _description: &TestStepDescription) {}

  fn report_step_wait(&mut self, description: &TestStepDescription) {
    if !self.parallel && self.scope_test_id == Some(description.parent_id) {
      self.force_report_step_wait(description);
    }
  }

  fn report_step_result(
    &mut self,
    desc: &TestStepDescription,
    result: &TestStepResult,
    elapsed: u64,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    match &result {
      TestStepResult::Ok => {
        self.summary.passed_steps += 1;
      }
      TestStepResult::Ignored => {
        self.summary.ignored_steps += 1;
      }
      TestStepResult::Failed(failure) => {
        self.summary.failed_steps += 1;
        self.summary.failures.push((
          TestFailureDescription {
            id: desc.id,
            name: common::format_test_step_ancestry(desc, tests, test_steps),
            origin: desc.origin.clone(),
            location: desc.location.clone(),
          },
          failure.clone(),
        ))
      }
    }

    if self.parallel {
      self.write_output_end();
      write!(
        &mut self.writer,
        "{} {} ...",
        colors::gray(format!(
          "{} =>",
          to_relative_path_or_remote_url(&self.cwd, &desc.origin)
        )),
        common::format_test_step_ancestry(desc, tests, test_steps)
      )
      .ok();
      self.in_new_line = false;
      self.scope_test_id = Some(desc.id);
      self.force_report_step_result(desc, result, elapsed);
    } else {
      let sibling_results =
        self.child_results_buffer.entry(desc.parent_id).or_default();
      if self.scope_test_id == Some(desc.id)
        || self.scope_test_id == Some(desc.parent_id)
      {
        let sibling_results = std::mem::take(sibling_results);
        self.force_report_step_result(desc, result, elapsed);
        // Flush buffered sibling results.
        for (desc, result, elapsed) in sibling_results.values() {
          self.force_report_step_result(desc, result, *elapsed);
        }
      } else {
        sibling_results
          .insert(desc.id, (desc.clone(), result.clone(), elapsed));
      }
    }
  }

  fn report_summary(
    &mut self,
    elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    self.write_output_end();
    common::report_summary(
      &mut self.writer,
      &self.cwd,
      &self.summary,
      elapsed,
      &self.failure_format_options,
    );
    if !self.repl {
      writeln!(&mut self.writer).ok();
    }
    self.in_new_line = true;
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    common::report_sigint(
      &mut self.writer,
      &self.cwd,
      tests_pending,
      tests,
      test_steps,
    );
    self.in_new_line = true;
  }

  fn report_completed(&mut self) {
    self.write_output_end();
    self.ended_tests = true;
  }

  fn flush_report(
    &mut self,
    _elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    self.writer.flush().ok();
    Ok(())
  }
}

/// Filters destructive ANSI escape sequences from user output while preserving
/// colors/styles (SGR sequences). This prevents user code from clearing the
/// screen, moving the cursor, or otherwise disrupting the test reporter output.
///
/// Returns `Cow::Borrowed` when no filtering is needed (fast path).
fn filter_destructive_ansi(input: &[u8]) -> Cow<'_, [u8]> {
  // Fast path: if no escape characters, BEL, or BS exist, return as-is
  if !input.iter().any(|&b| b == 0x1b || b == 0x07 || b == 0x08) {
    return Cow::Borrowed(input);
  }

  let mut output = Vec::with_capacity(input.len());
  let mut i = 0;

  while i < input.len() {
    match input[i] {
      // BEL and BS are stripped
      0x07 | 0x08 => {
        i += 1;
      }
      0x1b => {
        if i + 1 >= input.len() {
          // Trailing ESC at end of chunk — drop it
          i += 1;
          continue;
        }
        match input[i + 1] {
          // CSI sequence: ESC [
          b'[' => {
            let (is_sgr, seq_len) = parse_csi(&input[i..]);
            if is_sgr {
              // SGR (colors/styles) — keep it
              output.extend_from_slice(&input[i..i + seq_len]);
            }
            // Non-SGR CSI — strip it
            i += seq_len;
          }
          // OSC sequence: ESC ]
          b']' => {
            i += skip_osc(&input[i..]);
          }
          // DCS (ESC P), PM (ESC ^), APC (ESC _)
          b'P' | b'^' | b'_' => {
            i += skip_string_sequence(&input[i..]);
          }
          // Fe sequences (ESC + 0x40..=0x5F except [ ] P ^ _ handled above)
          // Fp sequences (ESC + 0x30..=0x3F, e.g., ESC 7, ESC 8)
          // Fs sequences (ESC + 0x60..=0x7E, e.g., ESC c RIS)
          0x30..=0x5F | 0x60..=0x7E => {
            i += 2;
          }
          // nF sequences: ESC + intermediate bytes (0x20..=0x2F) + final byte
          0x20..=0x2F => {
            i += 2;
            while i < input.len() && (0x20..=0x2F).contains(&input[i]) {
              i += 1;
            }
            // Skip the final byte
            if i < input.len() && (0x30..=0x7E).contains(&input[i]) {
              i += 1;
            }
          }
          // ESC followed by something unexpected — drop just the ESC
          _ => {
            i += 1;
          }
        }
      }
      // Normal byte — keep it
      b => {
        output.push(b);
        i += 1;
      }
    }
  }

  Cow::Owned(output)
}

/// Parses a CSI sequence starting at `data[0] == ESC, data[1] == '['`.
/// Returns `(is_sgr, total_length)` where `is_sgr` is true if the final byte
/// is `m` (SGR sequence) and there are no private-mode markers (`?`, `>`, `<`).
fn parse_csi(data: &[u8]) -> (bool, usize) {
  debug_assert!(data.len() >= 2 && data[0] == 0x1b && data[1] == b'[');

  let mut j = 2;
  let mut has_private_marker = false;

  // Check for private mode marker
  if j < data.len() && matches!(data[j], b'?' | b'>' | b'<') {
    has_private_marker = true;
    j += 1;
  }

  // Skip parameter bytes (0x30..=0x3F: digits, semicolons, etc.)
  while j < data.len() && (0x30..=0x3F).contains(&data[j]) {
    j += 1;
  }

  // Skip intermediate bytes (0x20..=0x2F)
  while j < data.len() && (0x20..=0x2F).contains(&data[j]) {
    j += 1;
  }

  // Final byte (0x40..=0x7E)
  if j < data.len() && (0x40..=0x7E).contains(&data[j]) {
    let final_byte = data[j];
    j += 1;
    let is_sgr = final_byte == b'm' && !has_private_marker;
    (is_sgr, j)
  } else {
    // Malformed/incomplete CSI — consume what we've seen
    (false, j)
  }
}

/// Skips an OSC sequence: ESC ] ... (terminated by BEL or ST).
/// ST is either ESC \\ (0x1b 0x5c) or the C1 code 0x9c.
fn skip_osc(data: &[u8]) -> usize {
  debug_assert!(data.len() >= 2 && data[0] == 0x1b && data[1] == b']');

  let mut j = 2;
  while j < data.len() {
    match data[j] {
      0x07 => return j + 1,          // BEL terminator
      0x9c => return j + 1,          // ST (C1)
      0x1b if j + 1 < data.len() && data[j + 1] == b'\\' => return j + 2, // ST (ESC \)
      _ => j += 1,
    }
  }
  // Unterminated — consume everything
  j
}

/// Skips a DCS/PM/APC string sequence (ESC P / ESC ^ / ESC _) terminated by ST.
fn skip_string_sequence(data: &[u8]) -> usize {
  debug_assert!(
    data.len() >= 2
      && data[0] == 0x1b
      && matches!(data[1], b'P' | b'^' | b'_')
  );

  let mut j = 2;
  while j < data.len() {
    match data[j] {
      0x9c => return j + 1,          // ST (C1)
      0x1b if j + 1 < data.len() && data[j + 1] == b'\\' => return j + 2, // ST (ESC \)
      _ => j += 1,
    }
  }
  // Unterminated — consume everything
  j
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn filter_destructive_ansi_plain_text() {
    let input = b"hello world";
    let result = filter_destructive_ansi(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"hello world");
  }

  #[test]
  fn filter_destructive_ansi_preserves_sgr_color() {
    let input = b"\x1b[31mred\x1b[0m";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"\x1b[31mred\x1b[0m");
  }

  #[test]
  fn filter_destructive_ansi_preserves_complex_sgr() {
    let input = b"\x1b[1;38;2;255;0;0mbold red\x1b[0m";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"\x1b[1;38;2;255;0;0mbold red\x1b[0m");
  }

  #[test]
  fn filter_destructive_ansi_strips_clear_screen() {
    let input = b"before\x1b[2Jafter";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_cursor_up() {
    let input = b"line\x1b[2Aup";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"lineup");
  }

  #[test]
  fn filter_destructive_ansi_strips_erase_line() {
    let input = b"text\x1b[Kmore";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"textmore");
  }

  #[test]
  fn filter_destructive_ansi_strips_cursor_position() {
    let input = b"start\x1b[10;20Hend";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"startend");
  }

  #[test]
  fn filter_destructive_ansi_strips_terminal_reset() {
    let input = b"before\x1bcafter";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_private_mode() {
    // Hide cursor
    let input = b"text\x1b[?25lmore";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"textmore");
  }

  #[test]
  fn filter_destructive_ansi_strips_osc_title() {
    let input = b"before\x1b]0;evil title\x07after";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_osc_with_st() {
    let input = b"before\x1b]0;title\x1b\\after";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_mixed_sequences() {
    // SGR red + clear screen + text + SGR reset
    let input = b"\x1b[31mred\x1b[2Jtext\x1b[0m";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"\x1b[31mredtext\x1b[0m");
  }

  #[test]
  fn filter_destructive_ansi_bel_and_bs_stripped() {
    let input = b"hello\x07world\x08!";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"helloworld!");
  }

  #[test]
  fn filter_destructive_ansi_preserves_whitespace() {
    let input = b"line1\nline2\ttab\rcarriage";
    let result = filter_destructive_ansi(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"line1\nline2\ttab\rcarriage");
  }

  #[test]
  fn filter_destructive_ansi_strips_alt_screen() {
    // Alt screen on and off
    let input = b"\x1b[?1049hcontent\x1b[?1049l";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"content");
  }

  #[test]
  fn filter_destructive_ansi_strips_dcs_sequence() {
    let input = b"before\x1bPsome data\x1b\\after";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_scroll_up() {
    let input = b"text\x1b[3Smore";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"textmore");
  }

  #[test]
  fn filter_destructive_ansi_trailing_esc() {
    let input = b"text\x1b";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"text");
  }

  #[test]
  fn filter_destructive_ansi_sgr_reset_bare() {
    // Bare ESC[m is equivalent to ESC[0m
    let input = b"\x1b[mtext";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"\x1b[mtext");
  }
}
