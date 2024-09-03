// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub struct KvConfig {
  pub(crate) max_write_key_size_bytes: usize,
  pub(crate) max_read_key_size_bytes: usize,
  pub(crate) max_value_size_bytes: usize,
  pub(crate) max_read_ranges: usize,
  pub(crate) max_read_entries: usize,
  pub(crate) max_checks: usize,
  pub(crate) max_mutations: usize,
  pub(crate) max_watched_keys: usize,
  pub(crate) max_total_mutation_size_bytes: usize,
  pub(crate) max_total_key_size_bytes: usize,
}

impl KvConfig {
  pub fn builder() -> KvConfigBuilder {
    KvConfigBuilder::default()
  }
}

#[derive(Default)]
pub struct KvConfigBuilder {
  max_write_key_size_bytes: Option<usize>,
  max_value_size_bytes: Option<usize>,
  max_read_ranges: Option<usize>,
  max_read_entries: Option<usize>,
  max_checks: Option<usize>,
  max_mutations: Option<usize>,
  max_watched_keys: Option<usize>,
  max_total_mutation_size_bytes: Option<usize>,
  max_total_key_size_bytes: Option<usize>,
}

impl KvConfigBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn max_write_key_size_bytes(
    &mut self,
    max_write_key_size_bytes: usize,
  ) -> &mut Self {
    self.max_write_key_size_bytes = Some(max_write_key_size_bytes);
    self
  }

  pub fn max_value_size_bytes(
    &mut self,
    max_value_size_bytes: usize,
  ) -> &mut Self {
    self.max_value_size_bytes = Some(max_value_size_bytes);
    self
  }

  pub fn max_read_ranges(&mut self, max_read_ranges: usize) -> &mut Self {
    self.max_read_ranges = Some(max_read_ranges);
    self
  }

  pub fn max_read_entries(&mut self, max_read_entries: usize) -> &mut Self {
    self.max_read_entries = Some(max_read_entries);
    self
  }

  pub fn max_checks(&mut self, max_checks: usize) -> &mut Self {
    self.max_checks = Some(max_checks);
    self
  }

  pub fn max_mutations(&mut self, max_mutations: usize) -> &mut Self {
    self.max_mutations = Some(max_mutations);
    self
  }

  pub fn max_watched_keys(&mut self, max_watched_keys: usize) -> &mut Self {
    self.max_watched_keys = Some(max_watched_keys);
    self
  }

  pub fn max_total_mutation_size_bytes(
    &mut self,
    max_total_mutation_size_bytes: usize,
  ) -> &mut Self {
    self.max_total_mutation_size_bytes = Some(max_total_mutation_size_bytes);
    self
  }

  pub fn max_total_key_size_bytes(
    &mut self,
    max_total_key_size_bytes: usize,
  ) -> &mut Self {
    self.max_total_key_size_bytes = Some(max_total_key_size_bytes);
    self
  }

  pub fn build(&self) -> KvConfig {
    const MAX_WRITE_KEY_SIZE_BYTES: usize = 2048;
    // range selectors can contain 0x00 or 0xff suffixes
    const MAX_READ_KEY_SIZE_BYTES: usize = MAX_WRITE_KEY_SIZE_BYTES + 1;
    const MAX_VALUE_SIZE_BYTES: usize = 65536;
    const MAX_READ_RANGES: usize = 10;
    const MAX_READ_ENTRIES: usize = 1000;
    const MAX_CHECKS: usize = 100;
    const MAX_MUTATIONS: usize = 1000;
    const MAX_WATCHED_KEYS: usize = 10;
    const MAX_TOTAL_MUTATION_SIZE_BYTES: usize = 800 * 1024;
    const MAX_TOTAL_KEY_SIZE_BYTES: usize = 80 * 1024;

    KvConfig {
      max_write_key_size_bytes: self
        .max_write_key_size_bytes
        .unwrap_or(MAX_WRITE_KEY_SIZE_BYTES),
      max_read_key_size_bytes: self
        .max_write_key_size_bytes
        .map(|x|
          // range selectors can contain 0x00 or 0xff suffixes
          x + 1)
        .unwrap_or(MAX_READ_KEY_SIZE_BYTES),
      max_value_size_bytes: self
        .max_value_size_bytes
        .unwrap_or(MAX_VALUE_SIZE_BYTES),
      max_read_ranges: self.max_read_ranges.unwrap_or(MAX_READ_RANGES),
      max_read_entries: self.max_read_entries.unwrap_or(MAX_READ_ENTRIES),
      max_checks: self.max_checks.unwrap_or(MAX_CHECKS),
      max_mutations: self.max_mutations.unwrap_or(MAX_MUTATIONS),
      max_watched_keys: self.max_watched_keys.unwrap_or(MAX_WATCHED_KEYS),
      max_total_mutation_size_bytes: self
        .max_total_mutation_size_bytes
        .unwrap_or(MAX_TOTAL_MUTATION_SIZE_BYTES),
      max_total_key_size_bytes: self
        .max_total_key_size_bytes
        .unwrap_or(MAX_TOTAL_KEY_SIZE_BYTES),
    }
  }
}
