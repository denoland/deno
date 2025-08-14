// Copyright 2018-2025 the Deno authors. MIT license.

mod r#gen;
mod structs;

use std::collections::BTreeSet;

pub use r#gen::UNSTABLE_ENV_VAR_NAMES;
pub use r#gen::UNSTABLE_FEATURES;
pub use structs::UnstableFeatureKind;

pub const JS_SOURCE: deno_core::FastStaticString =
  deno_core::ascii_str_include!("./gen.js");

pub type ExitCb = Box<dyn Fn(&str, &str) + Send + Sync>;

#[allow(clippy::print_stderr)]
#[allow(clippy::disallowed_methods)]
fn exit(feature: &str, api_name: &str) {
  eprintln!("Feature '{feature}' for '{api_name}' was not specified, exiting.");
  std::process::exit(70);
}

pub struct FeatureChecker {
  features: BTreeSet<&'static str>,
  exit_cb: ExitCb,
}

impl Default for FeatureChecker {
  fn default() -> Self {
    Self {
      features: BTreeSet::new(),
      exit_cb: Box::new(exit),
    }
  }
}

impl FeatureChecker {
  #[inline(always)]
  pub fn check(&self, feature: &str) -> bool {
    self.features.contains(feature)
  }

  pub fn enable_feature(&mut self, feature: &'static str) {
    let inserted = self.features.insert(feature);
    assert!(
      inserted,
      "Trying to enable a feature that is already enabled: {feature}",
    );
  }

  #[inline(always)]
  pub fn check_or_exit(&self, feature: &str, api_name: &str) {
    if !self.check(feature) {
      (self.exit_cb)(feature, api_name);
    }
  }

  pub fn set_exit_cb(&mut self, cb: ExitCb) {
    self.exit_cb = cb;
  }
}
