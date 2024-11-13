use std::sync::Arc;

use deno_core::error::AnyError;

use crate::args::Flags;
use crate::args::UpdateFlags;
use crate::factory::CliFactory;

pub async fn update(
  flags: Arc<Flags>,
  update_flags: UpdateFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let npm_resolver = factory.npm_resolver().await?;
  if let Some(npm_resolver) = npm_resolver.as_managed() {
    npm_resolver.ensure_top_level_package_json_install().await?;
    let old = npm_resolver
      .snapshot()
      .package_reqs()
      .keys()
      .cloned()
      .collect::<Vec<_>>();
    eprintln!("old: {old:?}");
    npm_resolver.set_package_reqs(&[]).await?;
    npm_resolver.set_package_reqs(&old).await?;
  }
  super::cache_deps::cache_top_level_deps(&factory, None).await?;
  Ok(())
}
