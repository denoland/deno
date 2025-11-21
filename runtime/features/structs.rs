// Copyright 2018-2025 the Deno authors. MIT license.

#[derive(Clone, Debug)]
pub enum UnstableFeatureKind {
  Cli,
  Runtime,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct UnstableFeatureDefinition {
  pub name: &'static str,
  pub flag_name: &'static str,
  pub help_text: &'static str,
  pub show_in_help: bool,
  pub id: i32,
  pub kind: UnstableFeatureKind,
}
