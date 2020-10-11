// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use js_sys::Array;
use mailparse::{addrparse, dateparse};
use serde_derive::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Serialize, Debug)]
pub struct DenoSingleAddr {
  // Reprenting SingleMailAddress
  pub displayName: Option<String>,
  pub addr: Option<String>,
  pub groupName: Option<String>,
  pub addrs: Option<Vec<DenoSingleAddr>>,
}

#[wasm_bindgen]
pub fn parse_date(data: &[u8]) -> Result<i64, JsValue> {
  let date = std::str::from_utf8(data).unwrap();
  return dateparse(date).map_err(|e| JsValue::from_str(&e.to_string()));
}

#[wasm_bindgen]
pub fn parse_addr(data: &[u8]) -> Result<Array, JsValue> {
  let addr = std::str::from_utf8(data).unwrap();
  return addrparse(addr)
    .map_err(|e| JsValue::from_str(&e.to_string()))
    .map(|list| {
      return list
        .into_inner()
        .iter()
        .map(|x| match x {
          mailparse::MailAddr::Single(i) => {
            let info = i.clone();
            return JsValue::from_serde(&DenoSingleAddr {
              displayName: info.display_name,
              addr: Some(info.addr),
              groupName: None,
              addrs: None,
            })
            .unwrap();
          }
          mailparse::MailAddr::Group(i) => {
            let info = i.clone();
            let addrs = info
              .addrs
              .iter()
              .map(|addr| {
                let cln = addr.clone();
                DenoSingleAddr {
                  displayName: cln.display_name,
                  addr: Some(cln.addr),
                  groupName: None,
                  addrs: None,
                }
              })
              .collect();
            return JsValue::from_serde(&DenoSingleAddr {
              groupName: Some(info.group_name),
              addrs: Some(addrs),
              displayName: None,
              addr: None,
            })
            .unwrap();
          }
        })
        .collect::<Vec<JsValue>>()
        .into_iter()
        .map(JsValue::from)
        .collect();
    });
}
