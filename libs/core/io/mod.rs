// Copyright 2018-2025 the Deno authors. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno which refer to various rust objects that need
// to be persisted between various ops. For example, network sockets are
// resources. Resources may or may not correspond to a real operating system
// file descriptor (hence the different name).

use std::future::Future;
use std::pin::Pin;

mod buffer_strategy;
mod buffers;
mod resource;
mod resource_handle;
mod resource_table;

pub use buffer_strategy::AdaptiveBufferStrategy;
pub use buffers::BufMutView;
pub use buffers::BufView;
pub use resource::Resource;
pub use resource::TransferredResource;
pub use resource_handle::ResourceHandle;
pub use resource_handle::ResourceHandleFd;
pub use resource_handle::ResourceHandleSocket;
pub use resource_table::ResourceError;
pub use resource_table::ResourceId;
pub use resource_table::ResourceTable;

/// Returned by resource shutdown methods
pub type AsyncResult<T> =
  Pin<Box<dyn Future<Output = Result<T, deno_error::JsErrorBox>>>>;

pub enum WriteOutcome {
  Partial { nwritten: usize, view: BufView },
  Full { nwritten: usize },
}

impl WriteOutcome {
  pub fn nwritten(&self) -> usize {
    match self {
      WriteOutcome::Partial { nwritten, .. } => *nwritten,
      WriteOutcome::Full { nwritten } => *nwritten,
    }
  }
}
