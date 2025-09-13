// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;

use deno_core::error::CoreError;
use deno_core::stats::RuntimeActivity;
use deno_core::stats::RuntimeActivityDiff;
use deno_core::stats::RuntimeActivityStats;
use deno_core::stats::RuntimeActivityStatsFactory;
use deno_core::stats::RuntimeActivityStatsFilter;
use deno_core::stats::RuntimeActivityType;
use deno_runtime::worker::MainWorker;

use super::poll_event_loop;

/// How many times we're allowed to spin the event loop before considering something a leak.
const MAX_SANITIZER_LOOP_SPINS: usize = 16;

#[derive(Default)]
struct TopLevelSanitizerStats {
  map: HashMap<(RuntimeActivityType, Cow<'static, str>), usize>,
}

fn get_sanitizer_item(
  activity: RuntimeActivity,
) -> (RuntimeActivityType, Cow<'static, str>) {
  let activity_type = activity.activity();
  match activity {
    RuntimeActivity::AsyncOp(_, _, name) => (activity_type, name.into()),
    RuntimeActivity::Resource(_, _, name) => (activity_type, name.into()),
    RuntimeActivity::Interval(_, _) => (activity_type, "".into()),
    RuntimeActivity::Timer(_, _) => (activity_type, "".into()),
  }
}

fn get_sanitizer_item_ref(
  activity: &RuntimeActivity,
) -> (RuntimeActivityType, Cow<'_, str>) {
  let activity_type = activity.activity();
  match activity {
    RuntimeActivity::AsyncOp(_, _, name) => (activity_type, (*name).into()),
    RuntimeActivity::Resource(_, _, name) => (activity_type, name.into()),
    RuntimeActivity::Interval(_, _) => (activity_type, "".into()),
    RuntimeActivity::Timer(_, _) => (activity_type, "".into()),
  }
}

pub struct TestSanitizerHelper {
  activity_stats: RuntimeActivityStatsFactory,
  activity_filter: RuntimeActivityStatsFilter,
  top_level_sanitizer_stats: TopLevelSanitizerStats,
}

impl TestSanitizerHelper {
  pub fn capture_stats(&self) -> RuntimeActivityStats {
    self.activity_stats.clone().capture(&self.activity_filter)
  }
}

pub fn create_test_sanitizer_helper(
  worker: &mut MainWorker,
) -> TestSanitizerHelper {
  let stats = worker.js_runtime.runtime_activity_stats_factory();
  let ops = worker.js_runtime.op_names();
  // These particular ops may start and stop independently of tests, so we just filter them out
  // completely.
  let op_id_host_recv_message = ops
    .iter()
    .position(|op| *op == "op_host_recv_message")
    .unwrap();
  let op_id_host_recv_ctrl = ops
    .iter()
    .position(|op| *op == "op_host_recv_ctrl")
    .unwrap();

  // For consistency between tests with and without sanitizers, we _always_ include
  // the actual sanitizer capture before and after a test, but a test that ignores resource
  // or op sanitization simply doesn't throw if one of these constraints is violated.
  let mut filter = RuntimeActivityStatsFilter::default();
  filter = filter.with_resources();
  filter = filter.with_ops();
  filter = filter.with_timers();
  filter = filter.omit_op(op_id_host_recv_ctrl as _);
  filter = filter.omit_op(op_id_host_recv_message as _);

  // Count the top-level stats so we can filter them out if they complete and restart within
  // a test.
  let top_level_stats = stats.clone().capture(&filter);
  let mut top_level = TopLevelSanitizerStats::default();
  for activity in top_level_stats.dump().active {
    top_level
      .map
      .entry(get_sanitizer_item(activity))
      .and_modify(|n| *n += 1)
      .or_insert(1);
  }

  TestSanitizerHelper {
    activity_stats: stats,
    activity_filter: filter,
    top_level_sanitizer_stats: top_level,
  }
}

/// The sanitizer must ignore ops, resources and timers that were started at the top-level, but
/// completed and restarted, replacing themselves with the same "thing". For example, if you run a
/// `Deno.serve` server at the top level and make fetch requests to it during the test, those ops
/// should not count as completed during the test because they are immediately replaced.
fn is_empty(
  top_level: &TopLevelSanitizerStats,
  diff: &RuntimeActivityDiff,
) -> bool {
  // If the diff is empty, return empty
  if diff.is_empty() {
    return true;
  }

  // If the # of appeared != # of disappeared, we can exit fast with not empty
  if diff.appeared.len() != diff.disappeared.len() {
    return false;
  }

  // If there are no top-level ops and !diff.is_empty(), we can exit fast with not empty
  if top_level.map.is_empty() {
    return false;
  }

  // Otherwise we need to calculate replacement for top-level stats. Sanitizers will not fire
  // if an op, resource or timer is replaced and has a corresponding top-level op.
  let mut map = HashMap::new();
  for item in &diff.appeared {
    let item = get_sanitizer_item_ref(item);
    let Some(n1) = top_level.map.get(&item) else {
      return false;
    };
    let n2 = map.entry(item).and_modify(|n| *n += 1).or_insert(1);
    // If more ops appeared than were created at the top-level, return false
    if *n2 > *n1 {
      return false;
    }
  }

  // We know that we replaced no more things than were created at the top-level. So now we just want
  // to make sure that whatever thing was created has a corresponding disappearance record.
  for item in &diff.disappeared {
    let item = get_sanitizer_item_ref(item);
    // If more things of this type disappeared than appeared, return false
    let Some(n1) = map.get_mut(&item) else {
      return false;
    };
    *n1 -= 1;
    if *n1 == 0 {
      map.remove(&item);
    }
  }

  // If everything is accounted for, we are empty
  map.is_empty()
}

pub async fn wait_for_activity_to_stabilize(
  worker: &mut MainWorker,
  helper: &TestSanitizerHelper,
  before_test_stats: RuntimeActivityStats,
  sanitize_ops: bool,
  sanitize_resources: bool,
) -> Result<Option<RuntimeActivityDiff>, CoreError> {
  // First, check to see if there's any diff at all. If not, just continue.
  let after_test_stats = helper.capture_stats();
  let mut diff =
    RuntimeActivityStats::diff(&before_test_stats, &after_test_stats);
  if is_empty(&helper.top_level_sanitizer_stats, &diff) {
    // No activity, so we return early
    return Ok(None);
  }

  // We allow for up to MAX_SANITIZER_LOOP_SPINS to get to a point where there is no difference.
  // TODO(mmastrac): We could be much smarter about this if we had the concept of "progress" in
  // an event loop tick. Ideally we'd be able to tell if we were spinning and doing nothing, or
  // spinning and resolving ops.
  for _ in 0..MAX_SANITIZER_LOOP_SPINS {
    // There was a diff, so let the event loop run once
    poll_event_loop(worker).await?;

    let after_test_stats = helper.capture_stats();
    diff = RuntimeActivityStats::diff(&before_test_stats, &after_test_stats);
    if is_empty(&helper.top_level_sanitizer_stats, &diff) {
      return Ok(None);
    }
  }

  if !sanitize_ops {
    diff
      .appeared
      .retain(|activity| !matches!(activity, RuntimeActivity::AsyncOp(..)));
    diff
      .disappeared
      .retain(|activity| !matches!(activity, RuntimeActivity::AsyncOp(..)));
  }
  if !sanitize_resources {
    diff
      .appeared
      .retain(|activity| !matches!(activity, RuntimeActivity::Resource(..)));
    diff
      .disappeared
      .retain(|activity| !matches!(activity, RuntimeActivity::Resource(..)));
  }

  // Since we don't have an option to disable timer sanitization, we use sanitize_ops == false &&
  // sanitize_resources == false to disable those.
  if !sanitize_ops && !sanitize_resources {
    diff.appeared.retain(|activity| {
      !matches!(
        activity,
        RuntimeActivity::Timer(..) | RuntimeActivity::Interval(..)
      )
    });
    diff.disappeared.retain(|activity| {
      !matches!(
        activity,
        RuntimeActivity::Timer(..) | RuntimeActivity::Interval(..)
      )
    });
  }

  Ok(if is_empty(&helper.top_level_sanitizer_stats, &diff) {
    None
  } else {
    Some(diff)
  })
}
