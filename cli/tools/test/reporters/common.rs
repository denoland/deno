// Copyright 2018-2025 the Deno authors. MIT license.

use super::fmt::format_test_error;
use super::fmt::to_relative_path_or_remote_url;
use super::*;

pub(super) fn format_test_step_ancestry(
  desc: &TestStepDescription,
  tests: &IndexMap<usize, TestDescription>,
  test_steps: &IndexMap<usize, TestStepDescription>,
) -> String {
  let root;
  let mut ancestor_names = vec![];
  let mut current_desc = desc;
  loop {
    if let Some(step_desc) = test_steps.get(&current_desc.parent_id) {
      ancestor_names.push(&step_desc.name);
      current_desc = step_desc;
    } else {
      root = tests.get(&current_desc.parent_id).unwrap();
      break;
    }
  }
  ancestor_names.reverse();
  let mut result = String::new();
  result.push_str(&root.name);
  result.push_str(" ... ");
  for name in ancestor_names {
    result.push_str(name);
    result.push_str(" ... ");
  }
  result.push_str(&desc.name);
  result
}

pub fn format_test_for_summary(
  cwd: &Url,
  desc: &TestFailureDescription,
) -> String {
  format!(
    "{} {}",
    &desc.name,
    colors::gray(format!(
      "=> {}:{}:{}",
      to_relative_path_or_remote_url(cwd, &desc.location.file_name),
      desc.location.line_number,
      desc.location.column_number
    ))
  )
}

pub fn format_test_step_for_summary(
  cwd: &Url,
  desc: &TestStepDescription,
  tests: &IndexMap<usize, TestDescription>,
  test_steps: &IndexMap<usize, TestStepDescription>,
) -> String {
  let long_name = format_test_step_ancestry(desc, tests, test_steps);
  format!(
    "{} {}",
    long_name,
    colors::gray(format!(
      "=> {}:{}:{}",
      to_relative_path_or_remote_url(cwd, &desc.location.file_name),
      desc.location.line_number,
      desc.location.column_number
    ))
  )
}

pub(super) fn report_sigint(
  writer: &mut dyn std::io::Write,
  cwd: &Url,
  tests_pending: &HashSet<usize>,
  tests: &IndexMap<usize, TestDescription>,
  test_steps: &IndexMap<usize, TestStepDescription>,
) {
  if tests_pending.is_empty() {
    return;
  }
  let mut formatted_pending = BTreeSet::new();
  for id in tests_pending {
    if let Some(desc) = tests.get(id) {
      formatted_pending.insert(format_test_for_summary(cwd, &desc.into()));
    }
    if let Some(desc) = test_steps.get(id) {
      formatted_pending
        .insert(format_test_step_for_summary(cwd, desc, tests, test_steps));
    }
  }
  writeln!(
    writer,
    "\n{} The following tests were pending:\n",
    colors::intense_blue("SIGINT")
  )
  .ok();
  for entry in formatted_pending {
    writeln!(writer, "{}", entry).ok();
  }
  writeln!(writer).ok();
}

pub(super) fn report_summary(
  writer: &mut dyn std::io::Write,
  cwd: &Url,
  summary: &TestSummary,
  elapsed: &Duration,
  options: &TestFailureFormatOptions,
) {
  if !summary.failures.is_empty() || !summary.uncaught_errors.is_empty() {
    #[allow(clippy::type_complexity)] // Type alias doesn't look better here
    let mut failures_by_origin: BTreeMap<
      String,
      (
        Vec<(&TestFailureDescription, &TestFailure)>,
        Option<&JsError>,
      ),
    > = BTreeMap::default();
    let mut failure_titles = vec![];
    for (description, failure) in &summary.failures {
      let (failures, _) = failures_by_origin
        .entry(description.origin.clone())
        .or_default();
      failures.push((description, failure));
    }

    for (origin, js_error) in &summary.uncaught_errors {
      let (_, uncaught_error) =
        failures_by_origin.entry(origin.clone()).or_default();
      let _ = uncaught_error.insert(js_error.as_ref());
    }

    // note: the trailing whitespace is intentional to get a red background
    writeln!(writer, "\n{}\n", colors::white_bold_on_red(" ERRORS ")).ok();
    for (origin, (failures, uncaught_error)) in failures_by_origin {
      for (description, failure) in failures {
        if !failure.hide_in_summary() {
          let failure_title = format_test_for_summary(cwd, description);
          writeln!(writer, "{}", &failure_title).ok();
          writeln!(
            writer,
            "{}: {}",
            colors::red_bold("error"),
            failure.format(options)
          )
          .ok();
          writeln!(writer).ok();
          failure_titles.push(failure_title);
        }
      }
      if let Some(js_error) = uncaught_error {
        let failure_title = format!(
          "{} (uncaught error)",
          to_relative_path_or_remote_url(cwd, &origin)
        );
        writeln!(writer, "{}", &failure_title).ok();
        writeln!(
          writer,
          "{}: {}",
          colors::red_bold("error"),
          format_test_error(js_error, options)
        )
        .ok();
        writeln!(writer, "This error was not caught from a test and caused the test runner to fail on the referenced module.").ok();
        writeln!(writer, "It most likely originated from a dangling promise, event/timeout handler or top-level code.").ok();
        writeln!(writer).ok();
        failure_titles.push(failure_title);
      }
    }
    // note: the trailing whitespace is intentional to get a red background
    writeln!(writer, "{}\n", colors::white_bold_on_red(" FAILURES ")).ok();
    for failure_title in failure_titles {
      writeln!(writer, "{failure_title}").ok();
    }
  }

  let status = if summary.has_failed() {
    colors::red("FAILED").to_string()
  } else {
    colors::green("ok").to_string()
  };

  let get_steps_text = |count: usize| -> String {
    if count == 0 {
      String::new()
    } else if count == 1 {
      " (1 step)".to_string()
    } else {
      format!(" ({count} steps)")
    }
  };

  let mut summary_result = String::new();

  write!(
    summary_result,
    "{} passed{} | {} failed{}",
    summary.passed,
    get_steps_text(summary.passed_steps),
    summary.failed,
    get_steps_text(summary.failed_steps),
  )
  .ok();

  let ignored_steps = get_steps_text(summary.ignored_steps);
  if summary.ignored > 0 || !ignored_steps.is_empty() {
    write!(
      summary_result,
      " | {} ignored{}",
      summary.ignored, ignored_steps
    )
    .ok();
  }

  if summary.measured > 0 {
    write!(summary_result, " | {} measured", summary.measured,).ok();
  }

  if summary.filtered_out > 0 {
    write!(summary_result, " | {} filtered out", summary.filtered_out).ok();
  };

  writeln!(
    writer,
    "\n{} | {} {}",
    status,
    summary_result,
    colors::gray(format!("({})", display::human_elapsed(elapsed.as_millis()))),
  )
  .ok();
}
