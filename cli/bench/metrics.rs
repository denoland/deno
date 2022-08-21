// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use google_storage1::api::Object;
use google_storage1::hyper;
use google_storage1::hyper_rustls;
use google_storage1::oauth2;
use google_storage1::Storage;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::Cursor;

static GIT_HASH: Lazy<String> = Lazy::new(|| {
  test_util::run_collect(&["git", "rev-parse", "HEAD"], None, None, None, true)
    .0
    .trim()
    .to_string()
});

#[derive(serde::Serialize)]
struct Metric<T: serde::Serialize> {
  name: String,
  value: T,
  sha1: String,
  #[serde(rename = "type")]
  type_: String,
  time: i64,
}

pub struct Reporter {
  wtr: csv::Writer<Vec<u8>>,
  gcloud_client: Option<Storage>,
}

impl Reporter {
  pub async fn new() -> Self {
    dotenv::dotenv().ok();
    let gcloud_client =
      match std::env::var("CI").map(|_| std::env::var("GOOGLE_SVC_KEY")) {
        Ok(Ok(key_str)) => {
          let secret = oauth2::parse_service_account_key(key_str)
            .expect("Failed to load service account key");
          let auth =
            oauth2::authenticator::ServiceAccountAuthenticator::builder(secret)
              .build()
              .await
              .unwrap();
          let client = hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::new()
              .with_native_roots()
              .https_or_http()
              .enable_http1()
              .enable_http2()
              .build(),
          );
          Some(Storage::new(client, auth))
        }
        _ => None,
      };
    Self {
      wtr: csv::Writer::from_writer(vec![]),
      gcloud_client,
    }
  }

  pub fn write_one<T: serde::Serialize>(
    &mut self,
    type_: &str,
    name: &str,
    value: T,
  ) {
    self
      .wtr
      .serialize(Metric {
        name: name.to_string(),
        type_: type_.to_string(),
        value,
        sha1: GIT_HASH.clone(),
        time: chrono::Utc::now().timestamp_millis(),
      })
      .unwrap();
  }

  pub fn write<T: serde::Serialize + Copy>(
    &mut self,
    type_: &str,
    hashmap: &HashMap<String, T>,
  ) {
    for (name, value) in hashmap {
      self.write_one(type_, name, *value);
    }
  }

  pub async fn submit(mut self) {
    self.wtr.flush().unwrap();
    if let Some(client) = self.gcloud_client.take() {
      let mut reader = Cursor::new(self.wtr.into_inner().unwrap());
      let object: Object = Object::default();
      client
        .objects()
        .insert(object, "deno_benchmark_data")
        .name(&format!("{}.csv", *GIT_HASH))
        .param("uploadType", "multipart")
        .upload(&mut reader, "text/csv".parse().unwrap())
        .await
        .unwrap();
    }
  }
}
