// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::error::anyhow::with_context;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use log::debug;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Result;
use std::path::PathBuf;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use crate::tools::fmt::format_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockfileV2Content {
  version: &'static str,
  // Mapping between URLs and their checksums
  remote: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct LockfileV2 {
  content: LockfileV2Content,
  pub filename: PathBuf,
  write: bool,
}

impl LockfileV2 {
  pub fn new(filename: Path, write: bool) -> Result<Lockfile> {
    let content = if write {
      LockfileV2Content {
        version: "2",
        remote: BTreeMap::new(),
      }
    } else {
      let s = std::fs::read_to_string(filename).with_context(|| format!("Unable to read lockfile at: {}", filename))?;
      serde_json::from_str(&s).context("Unable to parse lockfile contents")?
    };

    Ok(LockfileV2 {
      write,
      content,
      filename: filename.to_owned(),
    })
  }
}


#[cfg(test)]
mod tests {
  use super::*;

}
