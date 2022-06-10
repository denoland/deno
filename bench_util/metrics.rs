// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use influxdb_client::Client;
use influxdb_client::InfluxError;
use influxdb_client::Point;
use influxdb_client::TimestampOptions;
use once_cell::sync::OnceCell;

static CLIENT: OnceCell<Option<Client>> = OnceCell::new();

/// Send metrics to InfluxDB "benchmarks" bucket. This function is a no-op if the
/// `CI` environment variable is not set.
pub async fn submit(points: Vec<Point>) -> Result<(), InfluxError> {
  let client: &Option<Client> = CLIENT.get_or_init(|| {
    dotenv::dotenv().ok();
    // Only run on the CI
    if std::env::var("CI").is_err() {
      return None;
    }
    Some(
      Client::new(
        std::env::var("INFLUXDB_HOST").unwrap(),
        std::env::var("INFLUXDB_API_TOKEN").unwrap(),
      )
      .with_org(std::env::var("INFLUXDB_ORG").unwrap())
      .with_bucket("benchmarks"),
    )
  });

  if let Some(client) = client {
    client
      .insert_points(&points, TimestampOptions::None)
      .await?;
  }
  Ok(())
}
