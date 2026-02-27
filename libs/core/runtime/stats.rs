// Copyright 2018-2025 the Deno authors. MIT license.

use super::ContextState;
use super::op_driver::OpDriver;
use super::op_driver::OpInflightStats;
use crate::OpId;
use crate::OpState;
use crate::PromiseId;
use crate::ResourceId;
use bit_set::BitSet;
use serde::Serialize;
use serde::Serializer;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::Deref;
use std::rc::Rc;

type ActivityId = usize;

/// Fast, const no-trace collection of hashes.
const NO_TRACES: [BTreeMap<ActivityId, Rc<str>>;
  RuntimeActivityType::MAX_TYPE as usize] = [
  BTreeMap::new(),
  BTreeMap::new(),
  BTreeMap::new(),
  BTreeMap::new(),
];

#[derive(Default)]
pub struct RuntimeActivityTraces {
  enabled: Cell<bool>,
  traces: RefCell<
    [BTreeMap<ActivityId, Rc<str>>; RuntimeActivityType::MAX_TYPE as usize],
  >,
}

impl RuntimeActivityTraces {
  pub(crate) fn set_enabled(&self, enabled: bool) {
    self.enabled.set(enabled);
    if !enabled {
      *self.traces.borrow_mut() = Default::default();
    }
  }

  pub(crate) fn submit(
    &self,
    activity_type: RuntimeActivityType,
    id: ActivityId,
    trace: &str,
  ) {
    debug_assert_ne!(
      activity_type,
      RuntimeActivityType::Interval,
      "Use Timer for for timers and intervals"
    );
    self.traces.borrow_mut()[activity_type as usize].insert(id, trace.into());
  }

  pub(crate) fn complete(
    &self,
    activity_type: RuntimeActivityType,
    id: ActivityId,
  ) {
    self.traces.borrow_mut()[activity_type as usize].remove(&id);
  }

  pub fn is_enabled(&self) -> bool {
    self.enabled.get()
  }

  pub fn count(&self) -> usize {
    self.traces.borrow().len()
  }

  pub fn get_all(
    &self,
    mut f: impl FnMut(RuntimeActivityType, ActivityId, &str),
  ) {
    let traces = self.traces.borrow();
    for i in 0..RuntimeActivityType::MAX_TYPE {
      for (key, value) in traces[i as usize].iter() {
        f(RuntimeActivityType::from_u8(i), *key, value.as_ref())
      }
    }
  }

  pub fn capture(
    &self,
  ) -> [BTreeMap<ActivityId, Rc<str>>; RuntimeActivityType::MAX_TYPE as usize]
  {
    if self.is_enabled() {
      self.traces.borrow().clone()
    } else {
      NO_TRACES
    }
  }

  pub fn get<T>(
    &self,
    activity_type: RuntimeActivityType,
    id: ActivityId,
    f: impl FnOnce(Option<&str>) -> T,
  ) -> T {
    f(self.traces.borrow()[activity_type as u8 as usize]
      .get(&id)
      .map(|x| x.as_ref()))
  }
}

#[derive(Clone)]
pub struct RuntimeActivityStatsFactory {
  pub(super) context_state: Rc<ContextState>,
  pub(super) op_state: Rc<RefCell<OpState>>,
}

/// Selects the statistics that you are interested in capturing.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct RuntimeActivityStatsFilter {
  include_timers: bool,
  include_ops: bool,
  include_resources: bool,
  op_filter: BitSet,
}

impl RuntimeActivityStatsFilter {
  pub fn all() -> Self {
    RuntimeActivityStatsFilter {
      include_ops: true,
      include_resources: true,
      include_timers: true,
      op_filter: BitSet::default(),
    }
  }

  pub fn with_ops(mut self) -> Self {
    self.include_ops = true;
    self
  }

  pub fn with_resources(mut self) -> Self {
    self.include_resources = true;
    self
  }

  pub fn with_timers(mut self) -> Self {
    self.include_timers = true;
    self
  }

  pub fn omit_op(mut self, op: OpId) -> Self {
    self.op_filter.insert(op as _);
    self
  }

  pub fn is_empty(&self) -> bool {
    // This ensures we don't miss a newly-added field in the empty comparison
    let Self {
      include_ops,
      include_resources,
      include_timers,
      op_filter: _,
    } = self;
    !(*include_ops) && !(*include_resources) && !(*include_timers)
  }
}

impl RuntimeActivityStatsFactory {
  /// Capture the current runtime activity.
  pub fn capture(
    self,
    filter: &RuntimeActivityStatsFilter,
  ) -> RuntimeActivityStats {
    let resources = if filter.include_resources {
      let res = &self.op_state.borrow().resource_table;
      let mut resources = ResourceOpenStats {
        resources: Vec::with_capacity(res.len()),
      };
      for resource in res.names() {
        resources
          .resources
          .push((resource.0, resource.1.to_string()))
      }
      resources
    } else {
      ResourceOpenStats::default()
    };

    let timers = if filter.include_timers {
      let timer_count = self.context_state.timers.len();
      let mut timers = TimerStats {
        timers: Vec::with_capacity(timer_count),
        repeats: BitSet::with_capacity(timer_count),
      };
      for (timer_id, repeats, is_system_timer) in
        &self.context_state.timers.iter()
      {
        // Ignore system timer from stats
        if is_system_timer {
          continue;
        }

        if repeats {
          timers.repeats.insert(timers.timers.len());
        }
        timers.timers.push(timer_id as usize);
      }
      timers
    } else {
      TimerStats::default()
    };

    let (ops, activity_traces) = if filter.include_ops {
      let ops = self.context_state.pending_ops.stats(&filter.op_filter);
      let activity_traces = self.context_state.activity_traces.capture();
      (ops, activity_traces)
    } else {
      (Default::default(), Default::default())
    };

    RuntimeActivityStats {
      context_state: self.context_state.clone(),
      ops,
      activity_traces,
      resources,
      timers,
    }
  }
}

#[derive(Default)]
pub struct ResourceOpenStats {
  pub(super) resources: Vec<(u32, String)>,
}

#[derive(Default)]
pub struct TimerStats {
  pub(super) timers: Vec<usize>,
  /// `repeats` is a bitset that reports whether a given index in the ID array
  /// is an interval (if true) or a timer (if false).
  pub(super) repeats: BitSet,
}

/// Information about in-flight ops, open resources, active timers and other runtime-specific
/// data that can be used for test sanitization.
pub struct RuntimeActivityStats {
  context_state: Rc<ContextState>,
  pub(super) ops: OpInflightStats,
  pub(super) activity_traces: [BTreeMap<ActivityId, Rc<str>>; 4],
  pub(super) resources: ResourceOpenStats,
  pub(super) timers: TimerStats,
}

/// Contains a runtime activity (op, timer, resource, etc.) stack trace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct RuntimeActivityTrace(Rc<str>);

impl Serialize for RuntimeActivityTrace {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    self.0.as_ref().serialize(serializer)
  }
}

impl Display for RuntimeActivityTrace {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.0.as_ref())
  }
}

impl Deref for RuntimeActivityTrace {
  type Target = str;
  fn deref(&self) -> &Self::Target {
    self.0.as_ref()
  }
}

impl From<&Rc<str>> for RuntimeActivityTrace {
  fn from(value: &Rc<str>) -> Self {
    Self(value.clone())
  }
}

/// The type of runtime activity being tracked.
#[derive(Debug, Serialize)]
pub enum RuntimeActivity {
  /// An async op, including the promise ID and op name, with an optional trace.
  AsyncOp(PromiseId, Option<RuntimeActivityTrace>, &'static str),
  /// A resource, including the resource ID and name, with an optional trace.
  Resource(ResourceId, Option<RuntimeActivityTrace>, String),
  /// A timer, including the timer ID, with an optional trace.
  Timer(usize, Option<RuntimeActivityTrace>),
  /// An interval, including the interval ID, with an optional trace.
  Interval(usize, Option<RuntimeActivityTrace>),
}

impl RuntimeActivity {
  pub fn activity(&self) -> RuntimeActivityType {
    match self {
      Self::AsyncOp(..) => RuntimeActivityType::AsyncOp,
      Self::Resource(..) => RuntimeActivityType::Resource,
      Self::Timer(..) => RuntimeActivityType::Timer,
      Self::Interval(..) => RuntimeActivityType::Interval,
    }
  }
}

/// A data-less discriminant for [`RuntimeActivity`].
#[derive(
  Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize,
)]
#[repr(u8)]
pub enum RuntimeActivityType {
  AsyncOp,
  Resource,
  Timer,
  Interval,
}

impl RuntimeActivityType {
  const MAX_TYPE: u8 = 4;

  pub(crate) fn from_u8(value: u8) -> Self {
    match value {
      0 => Self::AsyncOp,
      1 => Self::Resource,
      2 => Self::Timer,
      3 => Self::Interval,
      _ => unreachable!(),
    }
  }
}

impl RuntimeActivityStats {
  fn trace_for(
    &self,
    activity_type: RuntimeActivityType,
    id: ActivityId,
  ) -> Option<RuntimeActivityTrace> {
    debug_assert_ne!(
      activity_type,
      RuntimeActivityType::Interval,
      "Use Timer for for timers and intervals"
    );
    self.activity_traces[activity_type as u8 as usize]
      .get(&id)
      .map(|x| x.into())
  }

  /// Capture the data within this [`RuntimeActivityStats`] as a [`RuntimeActivitySnapshot`]
  /// with details of activity.
  pub fn dump(&self) -> RuntimeActivitySnapshot {
    let has_traces = !self.activity_traces.is_empty();
    let mut v = Vec::with_capacity(
      self.ops.ops.len()
        + self.resources.resources.len()
        + self.timers.timers.len(),
    );
    let ops = &self.context_state.op_ctxs;
    if has_traces {
      for op in self.ops.ops.iter() {
        v.push(RuntimeActivity::AsyncOp(
          op.0,
          self.trace_for(RuntimeActivityType::AsyncOp, op.0 as _),
          ops[op.1 as usize].decl.name,
        ));
      }
    } else {
      for op in self.ops.ops.iter() {
        v.push(RuntimeActivity::AsyncOp(
          op.0,
          None,
          ops[op.1 as usize].decl.name,
        ));
      }
    }
    for resource in self.resources.resources.iter() {
      v.push(RuntimeActivity::Resource(
        resource.0,
        None,
        resource.1.clone(),
      ))
    }
    if has_traces {
      for i in 0..self.timers.timers.len() {
        let id = self.timers.timers[i];
        if self.timers.repeats.contains(i) {
          v.push(RuntimeActivity::Interval(
            id,
            self.trace_for(RuntimeActivityType::Timer, id),
          ));
        } else {
          v.push(RuntimeActivity::Timer(
            id,
            self.trace_for(RuntimeActivityType::Timer, id),
          ));
        }
      }
    } else {
      for i in 0..self.timers.timers.len() {
        if self.timers.repeats.contains(i) {
          v.push(RuntimeActivity::Interval(self.timers.timers[i], None));
        } else {
          v.push(RuntimeActivity::Timer(self.timers.timers[i], None));
        }
      }
    }
    RuntimeActivitySnapshot { active: v }
  }

  pub fn diff(before: &Self, after: &Self) -> RuntimeActivityDiff {
    let mut appeared = vec![];
    let mut disappeared = vec![];
    let ops = &before.context_state.op_ctxs;

    let mut a = BitSet::new();
    for op in after.ops.ops.iter() {
      a.insert(op.0 as usize);
    }
    for op in before.ops.ops.iter() {
      if a.remove(op.0 as usize) {
        // continuing op
      } else {
        // before, but not after
        disappeared.push(RuntimeActivity::AsyncOp(
          op.0,
          before.trace_for(RuntimeActivityType::AsyncOp, op.0 as _),
          ops[op.1 as usize].decl.name,
        ));
      }
    }
    for op in after.ops.ops.iter() {
      if a.contains(op.0 as usize) {
        // after but not before
        appeared.push(RuntimeActivity::AsyncOp(
          op.0,
          after.trace_for(RuntimeActivityType::AsyncOp, op.0 as _),
          ops[op.1 as usize].decl.name,
        ));
      }
    }

    let mut a = BitSet::new();
    for op in after.resources.resources.iter() {
      a.insert(op.0 as usize);
    }
    for op in before.resources.resources.iter() {
      if a.remove(op.0 as usize) {
        // continuing op
      } else {
        // before, but not after
        disappeared.push(RuntimeActivity::Resource(op.0, None, op.1.clone()));
      }
    }
    for op in after.resources.resources.iter() {
      if a.contains(op.0 as usize) {
        // after but not before
        appeared.push(RuntimeActivity::Resource(op.0, None, op.1.clone()));
      }
    }

    let mut a = BitSet::new();
    for timer in after.timers.timers.iter() {
      a.insert(*timer);
    }
    for index in 0..before.timers.timers.len() {
      let timer = before.timers.timers[index];
      if a.remove(timer) {
        // continuing op
      } else {
        // before, but not after
        if before.timers.repeats.contains(index) {
          disappeared.push(RuntimeActivity::Interval(
            timer,
            before.trace_for(RuntimeActivityType::Timer, timer),
          ));
        } else {
          disappeared.push(RuntimeActivity::Timer(
            timer,
            before.trace_for(RuntimeActivityType::Timer, timer),
          ));
        }
      }
    }
    for index in 0..after.timers.timers.len() {
      let timer = after.timers.timers[index];
      if a.contains(timer) {
        // after but not before
        if after.timers.repeats.contains(index) {
          appeared.push(RuntimeActivity::Interval(
            timer,
            after.trace_for(RuntimeActivityType::Timer, timer),
          ));
        } else {
          appeared.push(RuntimeActivity::Timer(
            timer,
            after.trace_for(RuntimeActivityType::Timer, timer),
          ));
        }
      }
    }

    RuntimeActivityDiff {
      appeared,
      disappeared,
    }
  }
}

#[derive(Debug, Serialize)]
pub struct RuntimeActivityDiff {
  pub appeared: Vec<RuntimeActivity>,
  pub disappeared: Vec<RuntimeActivity>,
}

impl RuntimeActivityDiff {
  pub fn is_empty(&self) -> bool {
    self.appeared.is_empty() && self.disappeared.is_empty()
  }
}

#[derive(Debug, Serialize)]
pub struct RuntimeActivitySnapshot {
  pub active: Vec<RuntimeActivity>,
}
