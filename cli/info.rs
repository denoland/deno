// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::media_type::serialize_media_type;
use crate::MediaType;
use crate::ModuleSpecifier;

use serde::Serialize;
use serde::Serializer;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// The core structure representing information about a specific "root" file in
/// a module graph.  This is used to represent information as part of the `info`
/// subcommand.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleGraphInfo {
  pub compiled: Option<PathBuf>,
  pub dep_count: usize,
  #[serde(serialize_with = "serialize_media_type")]
  pub file_type: MediaType,
  pub files: ModuleInfoMap,
  #[serde(skip_serializing)]
  pub info: ModuleInfo,
  pub local: PathBuf,
  pub map: Option<PathBuf>,
  pub module: ModuleSpecifier,
  pub total_size: usize,
}

impl fmt::Display for ModuleGraphInfo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(
      f,
      "{} {}",
      colors::bold("local:"),
      self.local.to_string_lossy()
    )?;
    writeln!(f, "{} {}", colors::bold("type:"), self.file_type)?;
    if let Some(ref compiled) = self.compiled {
      writeln!(
        f,
        "{} {}",
        colors::bold("compiled:"),
        compiled.to_string_lossy()
      )?;
    }
    if let Some(ref map) = self.map {
      writeln!(f, "{} {}", colors::bold("map:"), map.to_string_lossy())?;
    }
    writeln!(
      f,
      "{} {} unique {}",
      colors::bold("deps:"),
      self.dep_count,
      colors::gray(&format!(
        "(total {})",
        human_size(self.info.total_size.unwrap_or(0) as f64)
      ))
    )?;
    writeln!(f)?;
    writeln!(
      f,
      "{} {}",
      self.info.name,
      colors::gray(&format!("({})", human_size(self.info.size as f64)))
    )?;

    let dep_count = self.info.deps.len();
    for (idx, dep) in self.info.deps.iter().enumerate() {
      dep.write_info(f, "", idx == dep_count - 1)?;
    }

    Ok(())
  }
}

/// Represents a unique dependency within the graph of the the dependencies for
/// a given module.
#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInfo {
  pub deps: Vec<ModuleInfo>,
  pub name: ModuleSpecifier,
  pub size: usize,
  pub total_size: Option<usize>,
}

impl PartialOrd for ModuleInfo {
  fn partial_cmp(&self, other: &ModuleInfo) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for ModuleInfo {
  fn cmp(&self, other: &ModuleInfo) -> Ordering {
    self.name.to_string().cmp(&other.name.to_string())
  }
}

impl ModuleInfo {
  pub fn write_info(
    &self,
    f: &mut fmt::Formatter<'_>,
    prefix: &str,
    last: bool,
  ) -> fmt::Result {
    let sibling_connector = if last { '└' } else { '├' };
    let child_connector = if self.deps.is_empty() { '─' } else { '┬' };
    let totals = if self.total_size.is_some() {
      colors::gray(&format!(" ({})", human_size(self.size as f64)))
    } else {
      colors::gray(" *")
    };

    writeln!(
      f,
      "{} {}{}",
      colors::gray(&format!(
        "{}{}─{}",
        prefix, sibling_connector, child_connector
      )),
      self.name,
      totals
    )?;

    let mut prefix = prefix.to_string();
    if last {
      prefix.push(' ');
    } else {
      prefix.push('│');
    }
    prefix.push(' ');

    let dep_count = self.deps.len();
    for (idx, dep) in self.deps.iter().enumerate() {
      dep.write_info(f, &prefix, idx == dep_count - 1)?;
    }

    Ok(())
  }
}

/// A flat map of dependencies for a given module graph.
#[derive(Debug)]
pub struct ModuleInfoMap(pub HashMap<ModuleSpecifier, ModuleInfoMapItem>);

impl ModuleInfoMap {
  pub fn new(map: HashMap<ModuleSpecifier, ModuleInfoMapItem>) -> Self {
    ModuleInfoMap(map)
  }
}

impl Serialize for ModuleInfoMap {
  /// Serializes inner hash map which is ordered by the key
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let ordered: BTreeMap<_, _> =
      self.0.iter().map(|(k, v)| (k.to_string(), v)).collect();
    ordered.serialize(serializer)
  }
}

/// An entry in the `ModuleInfoMap` the provides the size of the module and
/// a vector of its dependencies, which should also be available as entries
/// in the map.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInfoMapItem {
  pub deps: Vec<ModuleSpecifier>,
  pub size: usize,
}

/// A function that converts a float to a string the represents a human
/// readable version of that number.
pub fn human_size(size: f64) -> String {
  let negative = if size.is_sign_positive() { "" } else { "-" };
  let size = size.abs();
  let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
  if size < 1_f64 {
    return format!("{}{}{}", negative, size, "B");
  }
  let delimiter = 1024_f64;
  let exponent = std::cmp::min(
    (size.ln() / delimiter.ln()).floor() as i32,
    (units.len() - 1) as i32,
  );
  let pretty_bytes = format!("{:.2}", size / delimiter.powi(exponent))
    .parse::<f64>()
    .unwrap()
    * 1_f64;
  let unit = units[exponent as usize];
  format!("{}{}{}", negative, pretty_bytes, unit)
}

#[cfg(test)]
mod test {
  use super::*;
  use deno_core::serde_json::json;

  #[test]
  fn human_size_test() {
    assert_eq!(human_size(16_f64), "16B");
    assert_eq!(human_size((16 * 1024) as f64), "16KB");
    assert_eq!(human_size((16 * 1024 * 1024) as f64), "16MB");
    assert_eq!(human_size(16_f64 * 1024_f64.powf(3.0)), "16GB");
    assert_eq!(human_size(16_f64 * 1024_f64.powf(4.0)), "16TB");
    assert_eq!(human_size(16_f64 * 1024_f64.powf(5.0)), "16PB");
    assert_eq!(human_size(16_f64 * 1024_f64.powf(6.0)), "16EB");
    assert_eq!(human_size(16_f64 * 1024_f64.powf(7.0)), "16ZB");
    assert_eq!(human_size(16_f64 * 1024_f64.powf(8.0)), "16YB");
  }

  fn get_fixture() -> ModuleGraphInfo {
    let spec_c =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a/b/c.ts")
        .unwrap();
    let spec_d =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a/b/c.ts")
        .unwrap();
    let deps = vec![ModuleInfo {
      deps: Vec::new(),
      name: spec_d.clone(),
      size: 12345,
      total_size: None,
    }];
    let info = ModuleInfo {
      deps,
      name: spec_c.clone(),
      size: 12345,
      total_size: Some(12345),
    };
    let mut items = HashMap::new();
    items.insert(
      spec_c,
      ModuleInfoMapItem {
        deps: vec![spec_d.clone()],
        size: 12345,
      },
    );
    items.insert(
      spec_d,
      ModuleInfoMapItem {
        deps: Vec::new(),
        size: 12345,
      },
    );
    let files = ModuleInfoMap(items);

    ModuleGraphInfo {
      compiled: Some(PathBuf::from("/a/b/c.js")),
      dep_count: 99,
      file_type: MediaType::TypeScript,
      files,
      info,
      local: PathBuf::from("/a/b/c.ts"),
      map: None,
      module: ModuleSpecifier::resolve_url_or_path(
        "https://deno.land/x/a/b/c.ts",
      )
      .unwrap(),
      total_size: 999999,
    }
  }

  #[test]
  fn test_module_graph_info_display() {
    let fixture = get_fixture();
    let actual = fixture.to_string();
    assert!(actual.contains(" /a/b/c.ts"));
    assert!(actual.contains(" 99 unique"));
    assert!(actual.contains("(12.06KB)"));
    assert!(actual.contains("\n\nhttps://deno.land/x/a/b/c.ts"));
  }

  #[test]
  fn test_module_graph_info_json() {
    let fixture = get_fixture();
    let actual = json!(fixture);
    assert_eq!(
      actual,
      json!({
        "compiled": "/a/b/c.js",
        "depCount": 99,
        "fileType": "TypeScript",
        "files": {
          "https://deno.land/x/a/b/c.ts":{
            "deps": [],
            "size": 12345
          }
        },
        "local": "/a/b/c.ts",
        "map": null,
        "module": "https://deno.land/x/a/b/c.ts",
        "totalSize": 999999
      })
    );
  }
}
