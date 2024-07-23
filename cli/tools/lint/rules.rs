use std::collections::HashSet;
use std::sync::Arc;

use deno_config::deno_json::ConfigFile;
use deno_config::deno_json::LintRulesConfig;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_lint::rules;
use deno_lint::rules::LintRule;

use crate::resolver::SloppyImportsResolver;

use super::no_sloppy_imports::NoSloppyImportsRule;

#[derive(Debug)]
pub struct ConfiguredRules {
  pub all_rule_names: HashSet<&'static str>,
  pub rules: Vec<Box<dyn LintRule>>,
  // cli specific rules
  pub no_slow_types: bool,
}

impl ConfiguredRules {
  pub fn incremental_cache_state(&self) -> impl std::hash::Hash {
    #[derive(Hash)]
    struct State {
      names_with_hash: Vec<(&'static str, u64)>,
      no_slow_types: bool,
    }

    // use a hash of the rule names in order to bust the cache
    let mut names_with_hash = self
      .rules
      .iter()
      .map(|r| (r.code(), r.state_hash()))
      .collect::<Vec<_>>();
    // ensure this is stable by sorting it
    names_with_hash.sort_unstable();
    State {
      names_with_hash,
      no_slow_types: self.no_slow_types,
    }
  }
}

pub struct LintRulesProvider {
  sloppy_imports_resolver: Option<Arc<SloppyImportsResolver>>,
  workspace_resolver: Option<Arc<WorkspaceResolver>>,
}

impl LintRulesProvider {
  pub fn new(
    sloppy_imports_resolver: Option<Arc<SloppyImportsResolver>>,
    workspace_resolver: Option<Arc<WorkspaceResolver>>,
  ) -> Self {
    Self {
      sloppy_imports_resolver,
      workspace_resolver,
    }
  }

  pub fn resolve_lint_rules_err_empty(
    &self,
    rules: LintRulesConfig,
    maybe_config_file: Option<&ConfigFile>,
  ) -> Result<ConfiguredRules, AnyError> {
    let lint_rules = self.resolve_lint_rules(rules, maybe_config_file);
    if lint_rules.rules.is_empty() {
      bail!("No rules have been configured")
    }
    Ok(lint_rules)
  }

  pub fn resolve_lint_rules(
    &self,
    rules: LintRulesConfig,
    maybe_config_file: Option<&ConfigFile>,
  ) -> ConfiguredRules {
    const NO_SLOW_TYPES_NAME: &str = "no-slow-types";
    let mut all_rules = deno_lint::rules::get_all_rules();
    all_rules.push(Box::new(NoSloppyImportsRule::new(
      self.sloppy_imports_resolver.clone(),
      self.workspace_resolver.clone(),
    )));
    all_rules.sort_by(|a, b| a.code().cmp(b.code()));
    let all_rule_names =
      all_rules.iter().map(|r| r.code()).collect::<HashSet<_>>();
    let implicit_no_slow_types =
      maybe_config_file.map(|c| c.is_package()).unwrap_or(false);
    let no_slow_types = implicit_no_slow_types
      && !rules
        .exclude
        .as_ref()
        .map(|exclude| exclude.iter().any(|i| i == NO_SLOW_TYPES_NAME))
        .unwrap_or(false);
    let rules = rules::filtered_rules(
      all_rules,
      rules
        .tags
        .or_else(|| Some(get_default_tags(maybe_config_file))),
      rules.exclude.map(|exclude| {
        exclude
          .into_iter()
          .filter(|c| c != NO_SLOW_TYPES_NAME)
          .collect()
      }),
      rules.include.map(|include| {
        include
          .into_iter()
          .filter(|c| c != NO_SLOW_TYPES_NAME)
          .collect()
      }),
    );
    ConfiguredRules {
      rules,
      all_rule_names,
      no_slow_types,
    }
  }
}

fn get_default_tags(maybe_config_file: Option<&ConfigFile>) -> Vec<String> {
  let mut tags = Vec::with_capacity(2);
  tags.push("recommended".to_string());
  if maybe_config_file.map(|c| c.is_package()).unwrap_or(false) {
    tags.push("jsr".to_string());
  }
  tags
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::args::LintRulesConfig;

  #[test]
  fn recommended_rules_when_no_tags_in_config() {
    let rules_config = LintRulesConfig {
      exclude: Some(vec!["no-debugger".to_string()]),
      include: None,
      tags: None,
    };
    let rules_provider = LintRulesProvider::new(None, None);
    let rules = rules_provider.resolve_lint_rules(rules_config, None);
    let mut rule_names = rules
      .rules
      .into_iter()
      .map(|r| r.code().to_string())
      .collect::<Vec<_>>();
    rule_names.sort();
    let mut recommended_rule_names = rules_provider
      .resolve_lint_rules(Default::default(), None)
      .rules
      .into_iter()
      .filter(|r| r.tags().iter().any(|t| *t == "recommended"))
      .map(|r| r.code().to_string())
      .filter(|n| n != "no-debugger")
      .collect::<Vec<_>>();
    recommended_rule_names.sort();
    assert_eq!(rule_names, recommended_rule_names);
  }
}
