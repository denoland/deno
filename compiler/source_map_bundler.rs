// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::Result;

use sourcemap::SourceMap;
use sourcemap::SourceMapBuilder;

pub struct SourceMapBundler {
  builder: SourceMapBuilder,
}

impl SourceMapBundler {
  pub fn new(file: Option<&str>) -> Self {
    SourceMapBundler {
      builder: SourceMapBuilder::new(file),
    }
  }

  pub fn append(&mut self, sm: &SourceMap, line_offset: usize) {
    for (idx, src) in sm.sources().enumerate() {
      let src_id = self.builder.add_source(src);
      self
        .builder
        .set_source_contents(src_id, sm.get_source_contents(idx as u32));
    }
    for token in sm.tokens() {
      self.builder.add(
        token.get_dst_line() + line_offset as u32,
        token.get_dst_col(),
        token.get_src_line(),
        token.get_src_col(),
        token.get_source(),
        token.get_name(),
      );
    }
  }

  pub fn append_from_str(&mut self, s: &str, line_offset: usize) -> Result<()> {
    let sm = SourceMap::from_reader(s.as_bytes())?;
    self.append(&sm, line_offset);
    Ok(())
  }

  pub fn into_sourcemap(self) -> SourceMap {
    self.builder.into_sourcemap()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_source_map_append_from_str() {
    let mut smb = SourceMapBundler::new(None);
    let input = r#"{
      "version": 3,
      "sources": ["coolstuff.js"],
      "names": ["x", "alert"],
      "mappings": "AAAA,GAAIA,GAAI,EACR,IAAIA,GAAK,EAAG,CACVC,MAAM"
    }"#;
    smb.append_from_str(input, 10).unwrap();
    let sm = smb.into_sourcemap();
    assert_eq!(sm.sources().count(), 1);
  }
}
