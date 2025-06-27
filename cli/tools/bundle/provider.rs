use std::sync::Arc;

use deno_core::error::AnyError;
use deno_runtime::ops::bundle::BundleProvider;

use crate::args::Flags;

pub struct CliBundleProvider {
  flags: Arc<Flags>,
}

impl CliBundleProvider {
  pub fn new(flags: Arc<Flags>) -> Self {
    Self { flags }
  }
}

#[async_trait::async_trait]
impl BundleProvider for CliBundleProvider {
  async fn bundle(
    &self,
    options: deno_runtime::ops::bundle::BundleOptions,
  ) -> Result<(), AnyError> {
    todo!()
  }
}
