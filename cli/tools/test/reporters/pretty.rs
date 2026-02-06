// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io::IsTerminal;

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
    } else {
      write!(&mut self.writer, "\r").ok();
      write!(
        &mut self.writer,
        "{}{} ...",
        "  ".repeat(description.level),
        description.name
      )
      .ok();
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
    } else if std::io::stdout().is_terminal() {
      // We believe the cursor is right after "test name ...", but external
      // output (e.g. from native addons writing directly to fd 1) may have
      // moved it. Use \r to return to column 0 and re-write the test name
      // so the result line is always intact. For normal tests this harmlessly
      // overwrites the same bytes. Only do this on a real terminal — on pipes
      // \r is a literal byte that would produce doubled output.
      write!(&mut self.writer, "\r").ok();
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

/// Strips destructive ANSI escape sequences from user output while preserving
/// SGR (color/style) sequences. Returns `Cow::Borrowed` when no filtering needed.
fn filter_destructive_ansi(input: &[u8]) -> Cow<'_, [u8]> {
  if !input
    .iter()
    .any(|&b| b == 0x1b || b == 0x07 || b == 0x08 || b == b'\r')
  {
    return Cow::Borrowed(input);
  }

  let mut out = Vec::with_capacity(input.len());
  let mut i = 0;

  while i < input.len() {
    match input[i] {
      0x07 | 0x08 => i += 1,
      // Strip standalone \r (line-overwrite), keep \r\n
      b'\r' if i + 1 < input.len() && input[i + 1] == b'\n' => {
        out.extend_from_slice(b"\r\n");
        i += 2;
      }
      b'\r' => i += 1,
      0x1b if i + 1 >= input.len() => i += 1,
      0x1b => {
        match input[i + 1] {
          b'[' => {
            let seq_end = skip_csi(&input[i..]);
            // Keep SGR sequences (final byte 'm', no private marker '?'/'>'/'<')
            let final_byte = input.get(i + seq_end - 1);
            let has_private = input
              .get(i + 2)
              .is_some_and(|&b| matches!(b, b'?' | b'>' | b'<'));
            if final_byte == Some(&b'm') && !has_private {
              out.extend_from_slice(&input[i..i + seq_end]);
            }
            i += seq_end;
          }
          // OSC/DCS/PM/APC: string sequences terminated by BEL/ST
          b']' | b'P' | b'^' | b'_' => i += skip_str_seq(&input[i..]),
          // Two-byte ESC sequences (Fe/Fp/Fs)
          0x30..=0x7E => i += 2,
          // nF: ESC + intermediate bytes (0x20..=0x2F) + final byte
          0x20..=0x2F => {
            i += 2;
            while i < input.len() && (0x20..=0x2F).contains(&input[i]) {
              i += 1;
            }
            if i < input.len() && (0x30..=0x7E).contains(&input[i]) {
              i += 1;
            }
          }
          _ => i += 1,
        }
      }
      b => {
        out.push(b);
        i += 1;
      }
    }
  }

  Cow::Owned(out)
}

/// Returns the length of a CSI sequence (`ESC [` params final-byte).
fn skip_csi(data: &[u8]) -> usize {
  let mut j = 2;
  if j < data.len() && matches!(data[j], b'?' | b'>' | b'<') {
    j += 1;
  }
  while j < data.len() && (0x30..=0x3F).contains(&data[j]) {
    j += 1;
  }
  while j < data.len() && (0x20..=0x2F).contains(&data[j]) {
    j += 1;
  }
  if j < data.len() && (0x40..=0x7E).contains(&data[j]) {
    j += 1;
  }
  j
}

/// Skips an OSC/DCS/PM/APC string sequence terminated by BEL, ST (ESC \), or 0x9c.
fn skip_str_seq(data: &[u8]) -> usize {
  let mut j = 2;
  while j < data.len() {
    match data[j] {
      0x07 | 0x9c => return j + 1,
      0x1b if data.get(j + 1) == Some(&b'\\') => return j + 2,
      _ => j += 1,
    }
  }
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
    let input = b"line1\nline2\ttab";
    let result = filter_destructive_ansi(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"line1\nline2\ttab");
  }

  #[test]
  fn filter_destructive_ansi_strips_standalone_cr() {
    // Standalone \r (used by progress bars to overwrite lines) is stripped
    let input = b"progress\roverwrite";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"progressoverwrite");
  }

  #[test]
  fn filter_destructive_ansi_preserves_crlf() {
    // \r\n line endings are preserved
    let input = b"line1\r\nline2\r\n";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"line1\r\nline2\r\n");
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
