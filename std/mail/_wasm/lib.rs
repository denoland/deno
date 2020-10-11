// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use mailparse::{addrparse, dateparse};
use wasm_bindgen::prelude::*;
use js_sys::Array;
use serde_derive::{Serialize, Deserialize};

#[derive(Serialize, Debug)]
pub struct DenoSingleAddr {
    // Reprenting SingleMailAddress
    pub display_name: Option<String>,
    pub addr: Option<String>,
}

#[wasm_bindgen]
pub fn parse_date(data: &[u8]) -> Result<i64, JsValue> {
    let date = std::str::from_utf8(data).unwrap();
    return dateparse(date).map_err(|e| JsValue::from_str(&e.to_string()))
}


#[wasm_bindgen]
pub fn parse_addr_single(data: &[u8]) -> Result<Array, JsValue> {
    let addr = std::str::from_utf8(data).unwrap();
    return addrparse(addr).map_err(|e| JsValue::from_str(&e.to_string()))
           .map(|list| {
            return list
                    .into_inner()
                    .iter()
                    .map(|x| {
                        match x {
                            mailparse::MailAddr::Single(i) => {
                                let info = i.clone();
                                return JsValue::from_serde(&DenoSingleAddr{
                                    display_name: info.display_name,
                                    addr: Some(info.addr),
                                }).unwrap()
                            },
                            _ => return JsValue::from_serde(&DenoSingleAddr {
                                addr: None,
                                display_name: None,
                            }).unwrap()
                        }
                    })
                    .collect::<Vec<JsValue>>().into_iter().map(JsValue::from).collect()
        })
    }
