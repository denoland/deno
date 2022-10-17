// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::ParseStatus;
use crate::Stream;
use std::cell::UnsafeCell;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tokio::sync::mpsc;

/// A set of data associated with a request that we need to share across the flash
/// thread and the JS thread.
#[derive(Debug)]
pub struct RequestStatesSharedWithJS {
  pub stream: Mutex<Stream>,
  pub detached: AtomicBool,
  /// A receiver to get notification about the data availability on the stream.
  /// If it's `None` that means we don't need to read more data.
  pub read_rx: Mutex<Option<mpsc::Receiver<()>>>,
  /// A sender to notify JS thread that some data is available on the stream.
  /// TODO(magurotuna): is it needed to be shared with JS?
  pub read_tx: Mutex<Option<mpsc::Sender<()>>>,
}

/// A set of data associated with a request that we don't need to share with the
/// JS thread.
#[derive(Debug)]
pub struct RequestStatesInFlash {
  pub header_parse_status: ParseStatus,
  pub parse_buffer: UnsafeCell<Vec<u8>>,
}
