// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use js_sys::Array;
use mailparse::{addrparse, dateparse};
use serde_derive::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DenoAddr {
  // Reprenting both SingleMailAddr and DoubleMailAddr
  pub display_name: Option<String>,
  pub addr: Option<String>,
  pub group_name: Option<String>,
  pub addrs: Option<Vec<DenoAddr>>,
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
            return JsValue::from_serde(&DenoAddr {
              display_name: info.display_name,
              addr: Some(info.addr),
              group_name: None,
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
                DenoAddr {
                  display_name: cln.display_name,
                  addr: Some(cln.addr),
                  group_name: None,
                  addrs: None,
                }
              })
              .collect();
            return JsValue::from_serde(&DenoAddr {
              group_name: Some(info.group_name),
              addrs: Some(addrs),
              display_name: None,
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
