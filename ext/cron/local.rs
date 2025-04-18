// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::OnceCell;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::env;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::unsync::spawn;
use deno_core::unsync::JoinHandle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::WeakSender;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

use crate::CronError;
use crate::CronHandle;
use crate::CronHandler;
use crate::CronSpec;

const MAX_CRONS: usize = 100;
const DISPATCH_CONCURRENCY_LIMIT: usize = 50;
const MAX_BACKOFF_MS: u32 = 60 * 60 * 1_000; // 1 hour
const MAX_BACKOFF_COUNT: usize = 5;
const DEFAULT_BACKOFF_SCHEDULE: [u32; 5] = [100, 1_000, 5_000, 30_000, 60_000];

pub struct LocalCronHandler {
  cron_schedule_tx: OnceCell<mpsc::Sender<(String, bool)>>,
  concurrency_limiter: Arc<Semaphore>,
  cron_loop_join_handle: OnceCell<JoinHandle<()>>,
  runtime_state: Rc<RefCell<RuntimeState>>,
}

struct RuntimeState {
  crons: HashMap<String, Cron>,
  scheduled_deadlines: BTreeMap<u64, Vec<String>>,
}

struct Cron {
  spec: CronSpec,
  next_tx: mpsc::WeakSender<()>,
  current_execution_retries: u32,
}

impl Cron {
  fn backoff_schedule(&self) -> &[u32] {
    self
      .spec
      .backoff_schedule
      .as_deref()
      .unwrap_or(&DEFAULT_BACKOFF_SCHEDULE)
  }
}

impl Default for LocalCronHandler {
  fn default() -> Self {
    Self::new()
  }
}

impl LocalCronHandler {
  pub fn new() -> Self {
    Self {
      cron_schedule_tx: OnceCell::new(),
      concurrency_limiter: Arc::new(Semaphore::new(DISPATCH_CONCURRENCY_LIMIT)),
      cron_loop_join_handle: OnceCell::new(),
      runtime_state: Rc::new(RefCell::new(RuntimeState {
        crons: HashMap::new(),
        scheduled_deadlines: BTreeMap::new(),
      })),
    }
  }

  async fn cron_loop(
    runtime_state: Rc<RefCell<RuntimeState>>,
    mut cron_schedule_rx: mpsc::Receiver<(String, bool)>,
  ) -> Result<(), CronError> {
    loop {
      let earliest_deadline = runtime_state
        .borrow()
        .scheduled_deadlines
        .keys()
        .next()
        .copied();

      let sleep_fut = if let Some(earliest_deadline) = earliest_deadline {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        if let Some(delta) = earliest_deadline.checked_sub(now) {
          tokio::time::sleep(std::time::Duration::from_millis(delta)).boxed()
        } else {
          std::future::ready(()).boxed()
        }
      } else {
        futures::future::pending().boxed()
      };

      let cron_to_schedule = tokio::select! {
        _ = sleep_fut => None,
        x = cron_schedule_rx.recv() => {
          if x.is_none() {
            return Ok(());
          };
          x
        }
      };

      // Schedule next execution of the cron if needed.
      if let Some((name, prev_success)) = cron_to_schedule {
        let mut runtime_state = runtime_state.borrow_mut();
        if let Some(cron) = runtime_state.crons.get_mut(&name) {
          let backoff_schedule = cron.backoff_schedule();
          let next_deadline = if !prev_success
            && cron.current_execution_retries < backoff_schedule.len() as u32
          {
            let backoff_ms =
              backoff_schedule[cron.current_execution_retries as usize];
            let now = chrono::Utc::now().timestamp_millis() as u64;
            cron.current_execution_retries += 1;
            now + backoff_ms as u64
          } else {
            let next_ts = compute_next_deadline(&cron.spec.cron_schedule)?;
            cron.current_execution_retries = 0;
            next_ts
          };
          runtime_state
            .scheduled_deadlines
            .entry(next_deadline)
            .or_default()
            .push(name.to_string());
        }
      }

      // Dispatch ready to execute crons.
      let crons_to_execute = {
        let mut runtime_state = runtime_state.borrow_mut();
        runtime_state.get_ready_crons()?
      };
      for (_, tx) in crons_to_execute {
        if let Some(tx) = tx.upgrade() {
          let _ = tx.send(()).await;
        }
      }
    }
  }
}

impl RuntimeState {
  fn get_ready_crons(
    &mut self,
  ) -> Result<Vec<(String, WeakSender<()>)>, CronError> {
    let now = chrono::Utc::now().timestamp_millis() as u64;

    let ready = {
      let to_remove = self
        .scheduled_deadlines
        .range(..=now)
        .map(|(ts, _)| *ts)
        .collect::<Vec<_>>();
      to_remove
        .iter()
        .flat_map(|ts| {
          self
            .scheduled_deadlines
            .remove(ts)
            .unwrap()
            .iter()
            .map(move |name| (*ts, name.clone()))
            .collect::<Vec<_>>()
        })
        .filter_map(|(_, name)| {
          self
            .crons
            .get(&name)
            .map(|c| (name.clone(), c.next_tx.clone()))
        })
        .collect::<Vec<_>>()
    };

    Ok(ready)
  }
}

#[async_trait(?Send)]
impl CronHandler for LocalCronHandler {
  type EH = CronExecutionHandle;

  fn create(&self, spec: CronSpec) -> Result<Self::EH, CronError> {
    // Ensure that the cron loop is started.
    self.cron_loop_join_handle.get_or_init(|| {
      let (cron_schedule_tx, cron_schedule_rx) =
        mpsc::channel::<(String, bool)>(1);
      self.cron_schedule_tx.set(cron_schedule_tx).unwrap();
      let runtime_state = self.runtime_state.clone();
      spawn(async move {
        LocalCronHandler::cron_loop(runtime_state, cron_schedule_rx)
          .await
          .unwrap();
      })
    });

    let mut runtime_state = self.runtime_state.borrow_mut();

    if runtime_state.crons.len() > MAX_CRONS {
      return Err(CronError::TooManyCrons);
    }
    if runtime_state.crons.contains_key(&spec.name) {
      return Err(CronError::AlreadyExists);
    }

    // Validate schedule expression.
    spec
      .cron_schedule
      .parse::<saffron::Cron>()
      .map_err(|_| CronError::InvalidCron)?;

    // Validate backoff_schedule.
    if let Some(backoff_schedule) = &spec.backoff_schedule {
      validate_backoff_schedule(backoff_schedule)?;
    }

    let (next_tx, next_rx) = mpsc::channel::<()>(1);
    let cron = Cron {
      spec: spec.clone(),
      next_tx: next_tx.downgrade(),
      current_execution_retries: 0,
    };
    runtime_state.crons.insert(spec.name.clone(), cron);

    Ok(CronExecutionHandle {
      name: spec.name.clone(),
      cron_schedule_tx: self.cron_schedule_tx.get().unwrap().clone(),
      concurrency_limiter: self.concurrency_limiter.clone(),
      runtime_state: Rc::downgrade(&self.runtime_state),
      inner: RefCell::new(Inner {
        next_rx: Some(next_rx),
        shutdown_tx: Some(next_tx),
        permit: None,
      }),
    })
  }
}

pub struct CronExecutionHandle {
  name: String,
  runtime_state: Weak<RefCell<RuntimeState>>,
  cron_schedule_tx: mpsc::Sender<(String, bool)>,
  concurrency_limiter: Arc<Semaphore>,
  inner: RefCell<Inner>,
}

struct Inner {
  next_rx: Option<mpsc::Receiver<()>>,
  shutdown_tx: Option<mpsc::Sender<()>>,
  permit: Option<OwnedSemaphorePermit>,
}

#[async_trait(?Send)]
impl CronHandle for CronExecutionHandle {
  async fn next(&self, prev_success: bool) -> Result<bool, CronError> {
    self.inner.borrow_mut().permit.take();

    if self
      .cron_schedule_tx
      .send((self.name.clone(), prev_success))
      .await
      .is_err()
    {
      return Ok(false);
    };

    let Some(mut next_rx) = self.inner.borrow_mut().next_rx.take() else {
      return Ok(false);
    };
    if next_rx.recv().await.is_none() {
      return Ok(false);
    };

    let permit = self.concurrency_limiter.clone().acquire_owned().await?;
    let mut inner = self.inner.borrow_mut();
    inner.next_rx = Some(next_rx);
    inner.permit = Some(permit);
    Ok(true)
  }

  fn close(&self) {
    if let Some(tx) = self.inner.borrow_mut().shutdown_tx.take() {
      drop(tx)
    }
    if let Some(runtime_state) = self.runtime_state.upgrade() {
      let mut runtime_state = runtime_state.borrow_mut();
      runtime_state.crons.remove(&self.name);
    }
  }
}

fn compute_next_deadline(cron_expression: &str) -> Result<u64, CronError> {
  let now = chrono::Utc::now();

  if let Ok(test_schedule) = env::var("DENO_CRON_TEST_SCHEDULE_OFFSET") {
    if let Ok(offset) = test_schedule.parse::<u64>() {
      return Ok(now.timestamp_millis() as u64 + offset);
    }
  }

  let cron = cron_expression
    .parse::<saffron::Cron>()
    .map_err(|_| CronError::InvalidCron)?;
  let Some(next_deadline) = cron.next_after(now) else {
    return Err(CronError::InvalidCron);
  };
  Ok(next_deadline.timestamp_millis() as u64)
}

fn validate_backoff_schedule(
  backoff_schedule: &[u32],
) -> Result<(), CronError> {
  if backoff_schedule.len() > MAX_BACKOFF_COUNT {
    return Err(CronError::InvalidBackoff);
  }
  if backoff_schedule.iter().any(|s| *s > MAX_BACKOFF_MS) {
    return Err(CronError::InvalidBackoff);
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_compute_next_deadline() {
    let now = chrono::Utc::now().timestamp_millis() as u64;
    assert!(compute_next_deadline("*/1 * * * *").unwrap() > now);
    assert!(compute_next_deadline("* * * * *").unwrap() > now);
    assert!(compute_next_deadline("bogus").is_err());
    assert!(compute_next_deadline("* * * * * *").is_err());
    assert!(compute_next_deadline("* * *").is_err());
  }
}
