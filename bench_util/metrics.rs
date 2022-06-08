// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::futures::prelude::*;
use influxdb2::models::DataPoint;
use influxdb2::Client;
use influxdb2::RequestError;
use once_cell::sync::OnceCell;

static CLIENT: OnceCell<Option<Client>> = OnceCell::new();

/// Send metrics to InfluxDB "benchmarks" bucket. This function is a no-op if the
/// `CI` environment variable is not set.
pub async fn submit(points: Vec<DataPoint>) -> Result<(), RequestError> {
  let client: &Option<Client> = CLIENT.get_or_init(|| {
    dotenv::dotenv().ok();
    // Only run on the CI
    if std::env::var("CI").is_err() {
      return None;
    }
    Some(Client::new(
      std::env::var("INFLUXDB_HOST").unwrap(),
      std::env::var("INFLUXDB_ORG").unwrap(),
      std::env::var("INFLUXDB_API_TOKEN").unwrap(),
    ))
  });

  if let Some(client) = client {
    client.write("benchmarks", stream::iter(points)).await?;
  }
  Ok(())
}
