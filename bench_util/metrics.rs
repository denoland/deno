// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::futures::prelude::*;
use influxdb2::models::DataPoint;
use influxdb2::Client;
use influxdb2::RequestError;

use once_cell::sync::Lazy;

static CLIENT: Lazy<Client> = Lazy::new(|| {
  Client::new("http://localhost:8086", "test", "BDOmgmajHvIRLQjF_w3lv1NedJ19UUz4snag9FZEViDQycQD3mOQbGoGZmtLvjBEZVY7HvwlSo62dW_oLceXZQ==")
});

pub async fn submit(points: Vec<DataPoint>) -> Result<(), RequestError> {
  CLIENT.write("test2", stream::iter(points)).await?;
  Ok(())
}
