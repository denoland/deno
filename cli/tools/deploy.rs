// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_runtime::WorkerExecutionMode;

use crate::args::jsr_api_url;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::RunFlags;
use crate::factory::CliFactory;
use crate::registry;
use crate::tools;

pub async fn deploy(
  flags: Arc<Flags>,
  roots: LibWorkerFactoryRoots,
) -> Result<i32, AnyError> {
  let cli_factory = CliFactory::from_flags(flags.clone());
  let client = cli_factory.http_client_provider().get_or_create()?;
  let registry_api_url = jsr_api_url();

  let response =
    registry::get_package(&client, registry_api_url, "deno", "deploy").await?;
  let res = registry::parse_response::<registry::Package>(response).await?;

  let mut flags = Arc::unwrap_or_clone(flags);
  flags.subcommand = DenoSubcommand::Run(RunFlags {
    // https://github.com/denoland/deploy-cli
    script: format!(
      "jsr:@deno/deploy@{}",
      res
        .latest_version
        .expect("expected @deno/deploy to be published")
    ),
    watch: None,
    bare: false,
  });

  tools::run::run_script(
    WorkerExecutionMode::Run,
    Arc::new(flags),
    None,
    None,
    roots,
  )
  .await
}
