// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use mailparse::{addrparse, dateparse};
use wasm_bindgen::prelude::*;
use js_sys::Array;

#[wasm_bindgen]
pub struct DenoAddr {
    // Reprenting SingleMailAddr
    display_name: Option<String>,
    addr: Option<String>,
    // Representing GroupMailAddr
    group_name: Option<String>,
    addrs: Option<Vec<mailparse::SingleInfo>>,
}

#[wasm_bindgen]
pub fn parse_date(data: &[u8]) -> Result<i64, JsValue> {
    let date = std::str::from_utf8(data).unwrap();
    return dateparse(date).map_err(|e| JsValue::from_str(&e.to_string()))
}


#[wasm_bindgen]
pub fn parse_addr(data: &[u8]) -> Result<Array, JsValue> {
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
                                DenoAddr{
                                    display_name: info.display_name,
                                    addr: Some(info.addr),
                                    addrs: None,
                                    group_name: None,
                                }
                            },
                            mailparse::MailAddr::Group(i) => {
                                let info = i.clone();
                                DenoAddr{
                                    group_name: Some(info.group_name),
                                    addrs: Some(info.addrs),
                                    addr: None,
                                    display_name: None,
                                }
                            }
                        }
                    })
                    .collect::<Vec<DenoAddr>>().into_iter().map(JsValue::from).collect()
        })
    }
