use std::sync::Arc;

use deno_core::error::AnyError;

use crate::args::{BundleFlags, Flags};

pub async fn bundle(
  flags: Arc<Flags>,
  bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  eprintln!("bundle");
  Ok(())
}
