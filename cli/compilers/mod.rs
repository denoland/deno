use deno::ErrBox;
use futures::Future;

pub mod js;
pub mod json;
pub mod ts;

#[derive(Debug, Clone)]
pub struct CompiledModule {
  pub code: String,
  pub name: String,
}

pub type CompiledModuleFuture =
  dyn Future<Item = CompiledModule, Error = ErrBox> + Send;
