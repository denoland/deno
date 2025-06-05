// Copyright 2018-2025 the Deno authors. MIT license.

pub use inner::LaxSingleProcessFsFlag;
pub use inner::LaxSingleProcessFsFlagSys;

#[cfg(not(target_arch = "wasm32"))]
mod inner {
  use std::path::PathBuf;
  use std::sync::Arc;
  use std::time::Duration;

  use parking_lot::Mutex;
  use sys_traits::FsFileLock;
  use sys_traits::FsMetadataValue;

  use crate::Reporter;

  #[sys_traits::auto_impl]
  pub trait LaxSingleProcessFsFlagSys:
    sys_traits::FsOpen
    + sys_traits::FsMetadata
    + sys_traits::FsRemoveFile
    + sys_traits::FsWrite
    + sys_traits::ThreadSleep
    + sys_traits::SystemTimeNow
    + Clone
    + Send
    + Sync
    + 'static
  {
  }

  struct PollFile<TSys: LaxSingleProcessFsFlagSys> {
    sys: TSys,
    file_path: PathBuf,
    count: usize,
  }

  impl<TSys: LaxSingleProcessFsFlagSys> Drop for PollFile<TSys> {
    fn drop(&mut self) {
      // cleanup the poll file so the node_modules folder is more
      // deterministic and so it doesn't end up in `deno compile`
      _ = self.sys.fs_remove_file(&self.file_path);
    }
  }

  impl<TSys: LaxSingleProcessFsFlagSys> PollFile<TSys> {
    pub fn new(sys: TSys, file_path: PathBuf) -> Self {
      Self {
        sys,
        file_path,
        count: 0,
      }
    }

    pub fn touch(&mut self) {
      self.count += 1;
      _ = self.sys.fs_write(&self.file_path, self.count.to_string());
    }
  }

  struct LaxSingleProcessFsFlagInner<TSys: LaxSingleProcessFsFlagSys> {
    file_path: PathBuf,
    fs_file: TSys::File,
    poll_file: Arc<Mutex<Option<PollFile<TSys>>>>,
  }

  impl<TSys: LaxSingleProcessFsFlagSys> Drop
    for LaxSingleProcessFsFlagInner<TSys>
  {
    fn drop(&mut self) {
      // kill the poll thread and clean up the poll file
      self.poll_file.lock().take();
      // release the file lock
      if let Err(err) = self.fs_file.fs_file_unlock() {
        log::debug!(
          "Failed releasing lock for {}. {:#}",
          self.file_path.display(),
          err
        );
      }
    }
  }

  /// A file system based flag that will attempt to synchronize multiple
  /// processes so they go one after the other. In scenarios where
  /// synchronization cannot be achieved, it will allow the current process
  /// to proceed.
  ///
  /// This should only be used in places where it's ideal for multiple
  /// processes to not update something on the file system at the same time,
  /// but it's not that big of a deal.
  pub struct LaxSingleProcessFsFlag<TSys: LaxSingleProcessFsFlagSys>(
    #[allow(dead_code)] Option<LaxSingleProcessFsFlagInner<TSys>>,
  );

  impl<TSys: LaxSingleProcessFsFlagSys> LaxSingleProcessFsFlag<TSys> {
    pub async fn lock(
      sys: &TSys,
      file_path: PathBuf,
      reporter: &impl Reporter,
      long_wait_message: &str,
    ) -> Self {
      log::debug!("Acquiring file lock at {}", file_path.display());
      let last_updated_path = file_path.with_extension("lock.poll");
      let start_instant = std::time::Instant::now();
      let mut open_options = sys_traits::OpenOptions::new();
      open_options.create = true;
      open_options.read = true;
      open_options.write = true;
      let open_result = sys.fs_open(&file_path, &open_options);

      match open_result {
        Ok(mut fs_file) => {
          let mut pb_update_guard = None;
          let mut error_count = 0;
          while error_count < 10 {
            let lock_result =
              fs_file.fs_file_try_lock(sys_traits::FsFileLockMode::Exclusive);
            let poll_file_update_ms = 100;
            match lock_result {
              Ok(_) => {
                log::debug!("Acquired file lock at {}", file_path.display());
                let mut poll_file =
                  PollFile::new(sys.clone(), last_updated_path);
                poll_file.touch();
                let poll_file = Arc::new(Mutex::new(Some(poll_file)));

                // Spawn a blocking task that will continually update a file
                // signalling the lock is alive. This is a fail safe for when
                // a file lock is never released. For example, on some operating
                // systems, if a process does not release the lock (say it's
                // killed), then the OS may release it at an indeterminate time
                //
                // This uses a blocking task because we use a single threaded
                // runtime and this is time sensitive so we don't want it to update
                // at the whims of whatever is occurring on the runtime thread.
                let sys = sys.clone();
                deno_unsync::spawn_blocking({
                  let poll_file = poll_file.clone();
                  move || loop {
                    sys
                      .thread_sleep(Duration::from_millis(poll_file_update_ms));
                    match &mut *poll_file.lock() {
                      Some(poll_file) => poll_file.touch(),
                      None => return,
                    }
                  }
                });

                return Self(Some(LaxSingleProcessFsFlagInner {
                  file_path,
                  fs_file,
                  poll_file,
                }));
              }
              Err(_) => {
                // show a message if it's been a while
                if pb_update_guard.is_none()
                  && start_instant.elapsed().as_millis() > 1_000
                {
                  let guard = reporter.on_blocking(long_wait_message);
                  pb_update_guard = Some(guard);
                }

                // sleep for a little bit
                tokio::time::sleep(Duration::from_millis(20)).await;

                // Poll the last updated path to check if it's stopped updating,
                // which is an indication that the file lock is claimed, but
                // was never properly released.
                match sys
                  .fs_metadata(&last_updated_path)
                  .and_then(|p| p.modified())
                {
                  Ok(last_updated_time) => {
                    let current_time = sys.sys_time_now();
                    match current_time.duration_since(last_updated_time) {
                      Ok(duration) => {
                        if duration.as_millis()
                          > (poll_file_update_ms * 2) as u128
                        {
                          // the other process hasn't updated this file in a long time
                          // so maybe it was killed and the operating system hasn't
                          // released the file lock yet
                          return Self(None);
                        } else {
                          error_count = 0; // reset
                        }
                      }
                      Err(_) => {
                        error_count += 1;
                      }
                    }
                  }
                  Err(_) => {
                    error_count += 1;
                  }
                }
              }
            }
          }

          drop(pb_update_guard); // explicit for clarity
          Self(None)
        }
        Err(err) => {
          log::debug!(
            "Failed to open file lock at {}. {:#}",
            file_path.display(),
            err
          );
          Self(None) // let the process through
        }
      }
    }
  }
}

#[cfg(target_arch = "wasm32")]
mod inner {
  use std::marker::PhantomData;
  use std::path::PathBuf;

  use crate::Reporter;

  // Don't bother locking the folder when installing via Wasm for now.
  // In the future, what we'd need is a way to spawn a thread (worker)
  // and have it reliably do the update of the .poll file
  #[sys_traits::auto_impl]
  pub trait LaxSingleProcessFsFlagSys: Clone + Send + Sync + 'static {}

  pub struct LaxSingleProcessFsFlag<TSys: LaxSingleProcessFsFlagSys> {
    _data: PhantomData<TSys>,
  }

  impl<TSys: LaxSingleProcessFsFlagSys> LaxSingleProcessFsFlag<TSys> {
    pub async fn lock(
      _sys: &TSys,
      _file_path: PathBuf,
      _reporter: &impl Reporter,
      _long_wait_message: &str,
    ) -> Self {
      Self {
        _data: Default::default(),
      }
    }
  }
}

#[allow(clippy::disallowed_methods)]
#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
  use std::sync::Arc;
  use std::time::Duration;

  use parking_lot::Mutex;
  use test_util::TempDir;
  use tokio::sync::Notify;

  use super::*;
  use crate::LogReporter;

  #[tokio::test]
  async fn lax_fs_lock_basic() {
    let temp_dir = TempDir::new();
    let lock_path = temp_dir.path().join("file.lock");
    let signal1 = Arc::new(Notify::new());
    let signal2 = Arc::new(Notify::new());
    let signal3 = Arc::new(Notify::new());
    let signal4 = Arc::new(Notify::new());
    tokio::spawn({
      let lock_path = lock_path.clone();
      let signal1 = signal1.clone();
      let signal2 = signal2.clone();
      let signal3 = signal3.clone();
      let signal4 = signal4.clone();
      let temp_dir = temp_dir.clone();
      async move {
        let flag = LaxSingleProcessFsFlag::lock(
          &sys_traits::impls::RealSys,
          lock_path.to_path_buf(),
          &LogReporter,
          "waiting",
        )
        .await;
        signal1.notify_one();
        signal2.notified().await;
        tokio::time::sleep(Duration::from_millis(10)).await; // give the other thread time to acquire the lock
        temp_dir.write("file.txt", "update1");
        signal3.notify_one();
        signal4.notified().await;
        drop(flag);
      }
    });
    let signal5 = Arc::new(Notify::new());
    tokio::spawn({
      let lock_path = lock_path.clone();
      let temp_dir = temp_dir.clone();
      let signal5 = signal5.clone();
      async move {
        signal1.notified().await;
        signal2.notify_one();
        let flag = LaxSingleProcessFsFlag::lock(
          &sys_traits::impls::RealSys,
          lock_path.to_path_buf(),
          &LogReporter,
          "waiting",
        )
        .await;
        temp_dir.write("file.txt", "update2");
        signal5.notify_one();
        drop(flag);
      }
    });

    signal3.notified().await;
    assert_eq!(temp_dir.read_to_string("file.txt"), "update1");
    signal4.notify_one();
    signal5.notified().await;
    assert_eq!(temp_dir.read_to_string("file.txt"), "update2");

    // ensure this is cleaned up
    assert!(!lock_path.with_extension("lock.poll").exists())
  }

  #[tokio::test]
  async fn lax_fs_lock_ordered() {
    let temp_dir = TempDir::new();
    let lock_path = temp_dir.path().join("file.lock");
    let output_path = temp_dir.path().join("output");
    let expected_order = Arc::new(Mutex::new(Vec::new()));
    let count = 10;
    let mut tasks = Vec::with_capacity(count);

    std::fs::write(&output_path, "").unwrap();

    for i in 0..count {
      let lock_path = lock_path.clone();
      let output_path = output_path.clone();
      let expected_order = expected_order.clone();
      tasks.push(tokio::spawn(async move {
        let flag = LaxSingleProcessFsFlag::lock(
          &sys_traits::impls::RealSys,
          lock_path.to_path_buf(),
          &LogReporter,
          "waiting",
        )
        .await;
        expected_order.lock().push(i.to_string());
        // be extremely racy
        let mut output = std::fs::read_to_string(&output_path).unwrap();
        if !output.is_empty() {
          output.push('\n');
        }
        output.push_str(&i.to_string());
        std::fs::write(&output_path, output).unwrap();
        drop(flag);
      }));
    }

    futures::future::join_all(tasks).await;
    let expected_output = expected_order.lock().join("\n");
    assert_eq!(
      std::fs::read_to_string(output_path).unwrap(),
      expected_output
    );
  }
}
