use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_config::deno_json::ConfigFile;
use deno_config::deno_json::LintRulesConfig;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_graph::FastCheckDiagnostic;
use deno_graph::ModuleGraph;
use deno_lint::rules::LintRule;

use crate::resolver::SloppyImportsResolver;

mod no_sloppy_imports;
mod no_slow_types;

// used for publishing
pub use no_slow_types::collect_no_slow_type_diagnostics;

pub trait CliGraphPackageLintRule: std::fmt::Debug {
  fn code(&self) -> &'static str;

  fn tags(&self) -> &'static [&'static str] {
    &[]
  }

  fn docs(&self) -> &'static str;

  // todo(dsherret): this should not return FastCheckDiagnostics
  fn lint(
    &self,
    graph: &ModuleGraph,
    entrypoints: &[ModuleSpecifier],
  ) -> Vec<FastCheckDiagnostic>;
}

#[derive(Debug)]
pub enum CliLintRule {
  Basic(Box<dyn LintRule>),
  Graph(Box<dyn CliGraphPackageLintRule>),
}

impl CliLintRule {
  pub fn code(&self) -> &'static str {
    match self {
      Self::Basic(rule) => rule.code(),
      Self::Graph(rule) => rule.code(),
    }
  }

  pub fn tags(&self) -> &'static [&'static str] {
    match self {
      Self::Basic(rule) => rule.tags(),
      Self::Graph(rule) => rule.tags(),
    }
  }

  pub fn docs(&self) -> &'static str {
    match self {
      Self::Basic(rule) => rule.docs(),
      Self::Graph(rule) => rule.docs(),
    }
  }
}

#[derive(Debug)]
pub struct ConfiguredRules {
  pub all_rule_names: HashSet<&'static str>,
  pub rules: Vec<CliLintRule>,
}

impl ConfiguredRules {
  pub fn incremental_cache_state(&self) -> Option<impl std::hash::Hash> {
    if self
      .rules
      .iter()
      .any(|r| r.code() == no_sloppy_imports::CODE)
    {
      // incremental cache cannot be determined easily, so don't use the cache
      return None;
    }

    // use a hash of the rule names in order to bust the cache
    let mut codes = self.rules.iter().map(|r| r.code()).collect::<Vec<_>>();
    // ensure this is stable by sorting it
    codes.sort_unstable();
    Some(codes)
  }
}

pub struct LintRuleProvider {
  sloppy_imports_resolver: Option<Arc<SloppyImportsResolver>>,
  workspace_resolver: Option<Arc<WorkspaceResolver>>,
}

impl LintRuleProvider {
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
    let deno_lint_rules = deno_lint::rules::get_all_rules();
    let cli_lint_rules = vec![CliLintRule::Basic(Box::new(
      no_sloppy_imports::NoSloppyImportsRule::new(
        self.sloppy_imports_resolver.clone(),
        self.workspace_resolver.clone(),
      ),
    ))];
    let cli_graph_rules =
      vec![CliLintRule::Graph(Box::new(no_slow_types::NoSlowTypesRule))];
    let mut all_rule_names = HashSet::with_capacity(
      deno_lint_rules.len() + cli_lint_rules.len() + cli_graph_rules.len(),
    );
    let all_rules = deno_lint_rules
      .into_iter()
      .map(|rule| CliLintRule::Basic(rule))
      .chain(cli_lint_rules.into_iter())
      .chain(cli_graph_rules.into_iter())
      .inspect(|rule| {
        all_rule_names.insert(rule.code());
      });
    let rules = filtered_rules(
      all_rules,
      rules
        .tags
        .or_else(|| Some(get_default_tags(maybe_config_file))),
      rules.exclude,
      rules.include,
    );
    ConfiguredRules {
      rules,
      all_rule_names,
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

fn filtered_rules(
  all_rules: impl Iterator<Item = CliLintRule>,
  maybe_tags: Option<Vec<String>>,
  maybe_exclude: Option<Vec<String>>,
  maybe_include: Option<Vec<String>>,
) -> Vec<CliLintRule> {
  let tags_set =
    maybe_tags.map(|tags| tags.into_iter().collect::<HashSet<_>>());

  let mut rules = all_rules
    .filter(|rule| {
      let mut passes = if let Some(tags_set) = &tags_set {
        rule
          .tags()
          .iter()
          .any(|t| tags_set.contains(&t.to_string()))
      } else {
        true
      };

      if let Some(includes) = &maybe_include {
        if includes.contains(&rule.code().to_owned()) {
          passes |= true;
        }
      }

      if let Some(excludes) = &maybe_exclude {
        if excludes.contains(&rule.code().to_owned()) {
          passes &= false;
        }
      }

      passes
    })
    .collect::<Vec<_>>();

  rules.sort_by_key(|r| r.code());

  rules
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
    let rules_provider = LintRuleProvider::new(None, None);
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
