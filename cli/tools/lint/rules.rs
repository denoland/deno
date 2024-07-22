use deno_config::deno_json::ConfigFile;
use deno_config::deno_json::LintRulesConfig;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_lint::rules::LintRule;

pub fn get_config_rules_err_empty(
  rules: LintRulesConfig,
  maybe_config_file: Option<&ConfigFile>,
) -> Result<ConfiguredRules, AnyError> {
  let lint_rules = get_configured_rules(rules, maybe_config_file);
  if lint_rules.rules.is_empty() {
    bail!("No rules have been configured")
  }
  Ok(lint_rules)
}

#[derive(Debug, Clone)]
pub struct ConfiguredRules {
  pub rules: Vec<Box<dyn LintRule>>,
  // cli specific rules
  pub no_slow_types: bool,
}

impl Default for ConfiguredRules {
  fn default() -> Self {
    get_configured_rules(Default::default(), None)
  }
}

impl ConfiguredRules {
  fn incremental_cache_state(&self) -> Vec<&str> {
    // use a hash of the rule names in order to bust the cache
    let mut names = self.rules.iter().map(|r| r.code()).collect::<Vec<_>>();
    // ensure this is stable by sorting it
    names.sort_unstable();
    if self.no_slow_types {
      names.push("no-slow-types");
    }
    names
  }
}

pub fn get_configured_rules(
  rules: LintRulesConfig,
  maybe_config_file: Option<&ConfigFile>,
) -> ConfiguredRules {
  const NO_SLOW_TYPES_NAME: &str = "no-slow-types";
  let implicit_no_slow_types =
    maybe_config_file.map(|c| c.is_package()).unwrap_or(false);
  let no_slow_types = implicit_no_slow_types
    && !rules
      .exclude
      .as_ref()
      .map(|exclude| exclude.iter().any(|i| i == NO_SLOW_TYPES_NAME))
      .unwrap_or(false);
  let rules = rules::get_filtered_rules(
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
    no_slow_types,
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
