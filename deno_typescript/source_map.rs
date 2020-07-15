// Copyright 2018-2020 the Deno author. All rights reserved. MIT license.

use sourcemap::Error;
use sourcemap::SourceMap;
use sourcemap::SourceMapBuilder;
use std::io::Read;
use std::io::Write;
use std::str;

pub struct SourceMapBundler {
  builder: SourceMapBuilder,
}

impl SourceMapBundler {
  pub fn new(file: Option<&str>) -> Self {
    SourceMapBundler {
      builder: SourceMapBuilder::new(file),
    }
  }

  pub fn append(&mut self, sm: SourceMap, line_offset: u32) {
    for (idx, src) in sm.sources().enumerate() {
      let src_id = self.builder.add_source(src);
      self
        .builder
        .set_source_contents(src_id, sm.get_source_contents(idx as u32));
    }
    for token in sm.tokens() {
      self.builder.add(
        token.get_dst_line() + line_offset,
        token.get_dst_col(),
        token.get_src_line(),
        token.get_src_col(),
        token.get_source(),
        token.get_name(),
      );
    }
  }

  pub fn append_from_reader<R: Read>(
    &mut self,
    rdr: R,
    line_offset: u32,
  ) -> Result<(), Error> {
    let sm = SourceMap::from_reader(rdr)?;
    self.append(sm, line_offset);
    Ok(())
  }

  pub fn append_from_slice(
    &mut self,
    slice: &[u8],
    line_offset: u32,
  ) -> Result<(), Error> {
    let sm = SourceMap::from_slice(slice)?;
    self.append(sm, line_offset);
    Ok(())
  }

  pub fn append_from_str(
    &mut self,
    s: &str,
    line_offset: u32,
  ) -> Result<(), Error> {
    let sm = SourceMap::from_reader(s.as_bytes())?;
    self.append(sm, line_offset);
    Ok(())
  }

  pub fn into_sourcemap(self) -> SourceMap {
    self.builder.into_sourcemap()
  }

  pub fn into_writer<W: Write>(self, w: W) -> Result<(), Error> {
    self.into_sourcemap().to_writer(w)?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_source_map_append_from_reader() {
    let mut smb = SourceMapBundler::new(None);
    let input: &[_] = b"{
      \"version\":3,
      \"sources\":[\"coolstuff.js\"],
      \"names\":[\"x\",\"alert\"],
      \"mappings\":\"AAAA,GAAIA,GAAI,EACR,IAAIA,GAAK,EAAG,CACVC,MAAM\"
    }";
    smb.append_from_reader(input, 10).unwrap();
    let mut actual: Vec<u8> = vec![];
    smb.into_writer(&mut actual).unwrap();
  }

  #[test]
  fn test_source_map_append_from_slice() {
    let mut smb = SourceMapBundler::new(None);
    let input: &[_] = b"{
      \"version\":3,
      \"sources\":[\"coolstuff.js\"],
      \"names\":[\"x\",\"alert\"],
      \"mappings\":\"AAAA,GAAIA,GAAI,EACR,IAAIA,GAAK,EAAG,CACVC,MAAM\"
    }";
    smb.append_from_slice(input, 10).unwrap();
    let mut actual: Vec<u8> = vec![];
    smb.into_writer(&mut actual).unwrap();
  }
}
