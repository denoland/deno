// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use monch::*;
use url::Url;

use self::ini::Key;
use self::ini::KeyValueOrSection;
use self::ini::Value;

mod ini;

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
  #[error("failed parsing npm registry url for scope '{scope}'")]
  UrlScope {
    scope: String,
    #[source]
    source: url::ParseError,
  },
  #[error("failed parsing npm registry url")]
  Url(#[source] url::ParseError),
}

pub type NpmRcParseError = monch::ParseErrorFailureError;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RegistryConfig {
  pub auth: Option<String>,
  pub auth_token: Option<String>,
  pub username: Option<String>,
  pub password: Option<String>,
  pub email: Option<String>,
  pub certfile: Option<String>,
  pub keyfile: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NpmRc {
  pub registry: Option<String>,
  pub scope_registries: HashMap<String, String>,
  pub registry_configs: HashMap<String, Arc<RegistryConfig>>,
}

impl NpmRc {
  pub fn parse(
    input: &str,
    get_env_var: &impl Fn(&str) -> Option<String>,
  ) -> Result<Self, NpmRcParseError> {
    let kv_or_sections = ini::parse_ini(input)?;
    let mut registry = None;
    let mut scope_registries: HashMap<String, String> = HashMap::new();
    let mut registry_configs: HashMap<String, RegistryConfig> = HashMap::new();

    for kv_or_section in kv_or_sections {
      match kv_or_section {
        KeyValueOrSection::KeyValue(kv) => {
          if let Key::Plain(key) = &kv.key {
            if let Some((left, right)) = key.rsplit_once(':') {
              if let Some(scope) = left.strip_prefix('@') {
                if right == "registry"
                  && let Value::String(text) = &kv.value
                {
                  let value = expand_vars(text, get_env_var);
                  scope_registries.insert(scope.to_string(), value);
                }
              } else if let Some(host_and_path) = left.strip_prefix("//")
                && let Value::String(text) = &kv.value
              {
                let value = expand_vars(text, get_env_var);
                let config = registry_configs
                  .entry(host_and_path.to_string())
                  .or_default();
                match right {
                  "_auth" => {
                    config.auth = Some(value);
                  }
                  "_authToken" => {
                    config.auth_token = Some(value);
                  }
                  "username" => {
                    config.username = Some(value);
                  }
                  "_password" => {
                    config.password = Some(value);
                  }
                  "email" => {
                    config.email = Some(value);
                  }
                  "certfile" => {
                    config.certfile = Some(value);
                  }
                  "keyfile" => {
                    config.keyfile = Some(value);
                  }
                  _ => {}
                }
              }
            } else if key == "registry"
              && let Value::String(text) = &kv.value
            {
              let value = expand_vars(text, get_env_var);
              registry = Some(value);
            }
          }
        }
        KeyValueOrSection::Section(_) => {
          // ignore
        }
      }
    }

    Ok(NpmRc {
      registry,
      scope_registries,
      registry_configs: registry_configs
        .into_iter()
        .map(|(k, v)| (k, Arc::new(v)))
        .collect(),
    })
  }

  pub fn as_resolved(
    &self,
    env_registry_url: &Url,
  ) -> Result<ResolvedNpmRc, ResolveError> {
    let mut scopes = HashMap::with_capacity(self.scope_registries.len());
    for scope in self.scope_registries.keys() {
      let (url, config) = self.registry_url_and_config_for_maybe_scope(
        Some(scope.as_str()),
        env_registry_url.as_str(),
      );
      let url = Url::parse(&url).map_err(|e| ResolveError::UrlScope {
        scope: scope.clone(),
        source: e,
      })?;
      scopes.insert(
        scope.clone(),
        RegistryConfigWithUrl {
          registry_url: url,
          config,
        },
      );
    }
    let (default_url, default_config) = self
      .registry_url_and_config_for_maybe_scope(None, env_registry_url.as_str());
    let default_url = Url::parse(&default_url).map_err(ResolveError::Url)?;
    Ok(ResolvedNpmRc {
      default_config: RegistryConfigWithUrl {
        registry_url: default_url,
        config: default_config,
      },
      scopes,
      registry_configs: self.registry_configs.clone(),
    })
  }

  fn registry_url_and_config_for_maybe_scope(
    &self,
    maybe_scope_name: Option<&str>,
    env_registry_url: &str,
  ) -> (String, Arc<RegistryConfig>) {
    let registry_url = maybe_scope_name
      .and_then(|scope| self.scope_registries.get(scope).map(|s| s.as_str()))
      .or(self.registry.as_deref())
      .unwrap_or(env_registry_url);

    let original_registry_url = if registry_url.ends_with('/') {
      Cow::Borrowed(registry_url)
    } else {
      Cow::Owned(format!("{}/", registry_url))
    };
    // https://example.com/ -> example.com/
    let Some((_, registry_url)) = original_registry_url
      .split_once("//")
      .filter(|(_, url)| !url.is_empty())
    else {
      return (
        original_registry_url.into_owned(),
        Arc::new(RegistryConfig::default()),
      );
    };
    let mut url: &str = registry_url;

    loop {
      if let Some(config) = self.registry_configs.get(url) {
        return (original_registry_url.into_owned(), config.clone());
      }
      let Some(next_slash_index) = url[..url.len() - 1].rfind('/') else {
        return (
          original_registry_url.into_owned(),
          Arc::new(RegistryConfig::default()),
        );
      };
      url = &url[..next_slash_index + 1];
    }
  }
}

fn get_scope_name(package_name: &str) -> Option<&str> {
  let no_at_pkg_name = package_name.strip_prefix('@')?;
  no_at_pkg_name.split_once('/').map(|(scope, _)| scope)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryConfigWithUrl {
  pub registry_url: Url,
  pub config: Arc<RegistryConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNpmRc {
  pub default_config: RegistryConfigWithUrl,
  pub scopes: HashMap<String, RegistryConfigWithUrl>,
  pub registry_configs: HashMap<String, Arc<RegistryConfig>>,
}

impl ResolvedNpmRc {
  pub fn get_registry_url(&self, package_name: &str) -> &Url {
    let Some(scope_name) = get_scope_name(package_name) else {
      return &self.default_config.registry_url;
    };

    match self.scopes.get(scope_name) {
      Some(registry_config) => &registry_config.registry_url,
      None => &self.default_config.registry_url,
    }
  }

  pub fn get_registry_config(
    &self,
    package_name: &str,
  ) -> &Arc<RegistryConfig> {
    let Some(scope_name) = get_scope_name(package_name) else {
      return &self.default_config.config;
    };

    match self.scopes.get(scope_name) {
      Some(registry_config) => &registry_config.config,
      None => &self.default_config.config,
    }
  }

  pub fn get_all_known_registries_urls(&self) -> Vec<Url> {
    let mut urls = Vec::with_capacity(1 + self.scopes.len());

    urls.push(self.default_config.registry_url.clone());
    for scope_config in self.scopes.values() {
      urls.push(scope_config.registry_url.clone());
    }
    urls
  }

  pub fn tarball_config(
    &self,
    tarball_url: &Url,
  ) -> Option<&Arc<RegistryConfig>> {
    // https://example.com/chalk.tgz -> example.com/.tgz
    let registry_url = tarball_url
      .as_str()
      .split_once("//")
      .map(|(_, right)| right)?;
    let mut best_match: Option<(&str, &Arc<RegistryConfig>)> = None;
    for (config_url, config) in &self.registry_configs {
      if registry_url.starts_with(config_url)
        && (best_match.is_none()
          || matches!(best_match, Some((current_config_url, _)) if config_url.len() > current_config_url.len()))
      {
        best_match = Some((config_url, config));
      }
    }
    best_match.map(|(_, config)| config)
  }
}

fn expand_vars(
  input: &str,
  get_env_var: &impl Fn(&str) -> Option<String>,
) -> String {
  fn escaped_char(input: &str) -> ParseResult<'_, char> {
    preceded(ch('\\'), next_char)(input)
  }

  fn env_var(input: &str) -> ParseResult<'_, &str> {
    let (input, _) = tag("${")(input)?;
    let (input, var_name) = take_while(|c| c != '}')(input)?;
    if var_name.chars().any(|c| matches!(c, '$' | '{' | '\\')) {
      return ParseError::backtrace();
    }
    let (input, _) = ch('}')(input)?;
    Ok((input, var_name))
  }

  let (input, results) = many0(or3(
    map(escaped_char, |c| c.to_string()),
    map(env_var, |var_name| {
      if let Some(var_value) = get_env_var(var_name) {
        var_value
      } else {
        format!("${{{}}}", var_name)
      }
    }),
    map(next_char, |c| c.to_string()),
  ))(input)
  .unwrap();
  assert!(input.is_empty());
  results.join("")
}

#[cfg(test)]
mod test {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn test_parse_basic() {
    // https://docs.npmjs.com/cli/v10/configuring-npm/npmrc#auth-related-configuration
    let npm_rc = NpmRc::parse(
      r#"
@myorg:registry=https://example.com/myorg
@another:registry=https://example.com/another
@example:registry=https://example.com/example
@yet_another:registry=https://yet.another.com/
//registry.npmjs.org/:_authToken=MYTOKEN
; would apply to both @myorg and @another
//example.com/:_authToken=MYTOKEN0
//example.com/:_auth=AUTH
//example.com/:username=USERNAME
//example.com/:_password=PASSWORD
//example.com/:email=EMAIL
//example.com/:certfile=CERTFILE
//example.com/:keyfile=KEYFILE
; would apply only to @myorg
//example.com/myorg/:_authToken=MYTOKEN1
; would apply only to @another
//example.com/another/:_authToken=MYTOKEN2
; this should not apply to `@yet_another`, because the URL contains the name of the scope
; and not the URL of the registry root specified above
//yet.another.com/yet_another/:_authToken=MYTOKEN3
registry=https://registry.npmjs.org/
"#,
      &|_| None,
    )
    .unwrap();
    assert_eq!(
      npm_rc,
      NpmRc {
        registry: Some("https://registry.npmjs.org/".to_string()),
        scope_registries: HashMap::from([
          ("myorg".to_string(), "https://example.com/myorg".to_string()),
          (
            "another".to_string(),
            "https://example.com/another".to_string()
          ),
          (
            "example".to_string(),
            "https://example.com/example".to_string()
          ),
          (
            "yet_another".to_string(),
            "https://yet.another.com/".to_string()
          ),
        ]),
        registry_configs: HashMap::from([
          (
            "example.com/".to_string(),
            Arc::new(RegistryConfig {
              auth: Some("AUTH".to_string()),
              auth_token: Some("MYTOKEN0".to_string()),
              username: Some("USERNAME".to_string()),
              password: Some("PASSWORD".to_string()),
              email: Some("EMAIL".to_string()),
              certfile: Some("CERTFILE".to_string()),
              keyfile: Some("KEYFILE".to_string()),
            })
          ),
          (
            "example.com/another/".to_string(),
            Arc::new(RegistryConfig {
              auth_token: Some("MYTOKEN2".to_string()),
              ..Default::default()
            })
          ),
          (
            "example.com/myorg/".to_string(),
            Arc::new(RegistryConfig {
              auth_token: Some("MYTOKEN1".to_string()),
              ..Default::default()
            })
          ),
          (
            "yet.another.com/yet_another/".to_string(),
            Arc::new(RegistryConfig {
              auth_token: Some("MYTOKEN3".to_string()),
              ..Default::default()
            })
          ),
          (
            "registry.npmjs.org/".to_string(),
            Arc::new(RegistryConfig {
              auth_token: Some("MYTOKEN".to_string()),
              ..Default::default()
            })
          ),
        ])
      }
    );

    let resolved_npm_rc = npm_rc
      .as_resolved(&Url::parse("https://deno.land/npm/").unwrap())
      .unwrap();
    assert_eq!(
      resolved_npm_rc,
      ResolvedNpmRc {
        default_config: RegistryConfigWithUrl {
          registry_url: Url::parse("https://registry.npmjs.org/").unwrap(),
          config: Arc::new(RegistryConfig {
            auth_token: Some("MYTOKEN".to_string()),
            ..Default::default()
          }),
        },
        scopes: HashMap::from([
          (
            "myorg".to_string(),
            RegistryConfigWithUrl {
              registry_url: Url::parse("https://example.com/myorg/").unwrap(),
              config: Arc::new(RegistryConfig {
                auth_token: Some("MYTOKEN1".to_string()),
                ..Default::default()
              })
            }
          ),
          (
            "another".to_string(),
            RegistryConfigWithUrl {
              registry_url: Url::parse("https://example.com/another/").unwrap(),
              config: Arc::new(RegistryConfig {
                auth_token: Some("MYTOKEN2".to_string()),
                ..Default::default()
              })
            }
          ),
          (
            "example".to_string(),
            RegistryConfigWithUrl {
              registry_url: Url::parse("https://example.com/example/").unwrap(),
              config: Arc::new(RegistryConfig {
                auth: Some("AUTH".to_string()),
                auth_token: Some("MYTOKEN0".to_string()),
                username: Some("USERNAME".to_string()),
                password: Some("PASSWORD".to_string()),
                email: Some("EMAIL".to_string()),
                certfile: Some("CERTFILE".to_string()),
                keyfile: Some("KEYFILE".to_string()),
              })
            }
          ),
          (
            "yet_another".to_string(),
            RegistryConfigWithUrl {
              registry_url: Url::parse("https://yet.another.com/").unwrap(),
              config: Default::default()
            }
          ),
        ]),
        registry_configs: npm_rc.registry_configs.clone(),
      }
    );

    // no matching scoped package
    {
      let registry_url = resolved_npm_rc.get_registry_url("test");
      let config = resolved_npm_rc.get_registry_config("test");
      assert_eq!(registry_url.as_str(), "https://registry.npmjs.org/");
      assert_eq!(config.auth_token, Some("MYTOKEN".to_string()));
    }
    // matching scoped package
    {
      let registry_url = resolved_npm_rc.get_registry_url("@example/pkg");
      let config = resolved_npm_rc.get_registry_config("@example/pkg");
      assert_eq!(registry_url.as_str(), "https://example.com/example/");
      assert_eq!(config.auth_token, Some("MYTOKEN0".to_string()));
    }
    // matching scoped package with specific token
    {
      let registry_url = resolved_npm_rc.get_registry_url("@myorg/pkg");
      let config = resolved_npm_rc.get_registry_config("@myorg/pkg");
      assert_eq!(registry_url.as_str(), "https://example.com/myorg/");
      assert_eq!(config.auth_token, Some("MYTOKEN1".to_string()));
    }
    // This should not return the token - the configuration is borked for `@yet_another` scope -
    // it defines the registry url as root + scope_name and instead it should be matching the
    // registry root.
    {
      let registry_url = resolved_npm_rc.get_registry_url("@yet_another/pkg");
      let config = resolved_npm_rc.get_registry_config("@yet_another/pkg");
      assert_eq!(registry_url.as_str(), "https://yet.another.com/");
      assert_eq!(config.auth_token, None);
    }

    assert_eq!(
      resolved_npm_rc.get_registry_url("@deno/test").as_str(),
      "https://registry.npmjs.org/"
    );
    assert_eq!(
      resolved_npm_rc
        .get_registry_config("@deno/test")
        .auth_token
        .as_ref()
        .unwrap(),
      "MYTOKEN"
    );

    assert_eq!(
      resolved_npm_rc.get_registry_url("@myorg/test").as_str(),
      "https://example.com/myorg/"
    );
    assert_eq!(
      resolved_npm_rc
        .get_registry_config("@myorg/test")
        .auth_token
        .as_ref()
        .unwrap(),
      "MYTOKEN1"
    );

    assert_eq!(
      resolved_npm_rc.get_registry_url("@another/test").as_str(),
      "https://example.com/another/"
    );
    assert_eq!(
      resolved_npm_rc
        .get_registry_config("@another/test")
        .auth_token
        .as_ref()
        .unwrap(),
      "MYTOKEN2"
    );

    assert_eq!(
      resolved_npm_rc.get_registry_url("@example/test").as_str(),
      "https://example.com/example/"
    );
    let config = resolved_npm_rc.get_registry_config("@example/test");
    assert_eq!(config.auth.as_ref().unwrap(), "AUTH");
    assert_eq!(config.auth_token.as_ref().unwrap(), "MYTOKEN0");
    assert_eq!(config.username.as_ref().unwrap(), "USERNAME");
    assert_eq!(config.password.as_ref().unwrap(), "PASSWORD");
    assert_eq!(config.email.as_ref().unwrap(), "EMAIL");
    assert_eq!(config.certfile.as_ref().unwrap(), "CERTFILE");
    assert_eq!(config.keyfile.as_ref().unwrap(), "KEYFILE");

    // tarball uri
    {
      assert_eq!(
        resolved_npm_rc
          .tarball_config(
            &Url::parse("https://example.com/example/chalk.tgz").unwrap(),
          )
          .unwrap()
          .auth_token
          .as_ref()
          .unwrap(),
        "MYTOKEN0"
      );
      assert_eq!(
        resolved_npm_rc
          .tarball_config(
            &Url::parse("https://example.com/myorg/chalk.tgz").unwrap(),
          )
          .unwrap()
          .auth_token
          .as_ref()
          .unwrap(),
        "MYTOKEN1"
      );
      assert_eq!(
        resolved_npm_rc
          .tarball_config(
            &Url::parse("https://example.com/another/chalk.tgz").unwrap(),
          )
          .unwrap()
          .auth_token
          .as_ref()
          .unwrap(),
        "MYTOKEN2"
      );
      assert_eq!(
        resolved_npm_rc.tarball_config(
          &Url::parse("https://yet.another.com/example/chalk.tgz").unwrap(),
        ),
        None,
      );
      assert_eq!(
        resolved_npm_rc
          .tarball_config(
            &Url::parse(
              "https://yet.another.com/yet_another/example/chalk.tgz"
            )
            .unwrap(),
          )
          .unwrap()
          .auth_token
          .as_ref()
          .unwrap(),
        "MYTOKEN3"
      );
    }
  }

  #[test]
  fn test_parse_env_vars() {
    let npm_rc = NpmRc::parse(
      r#"
@myorg:registry=${VAR_FOUND}
@another:registry=${VAR_NOT_FOUND}
@a:registry=\${VAR_FOUND}
//registry.npmjs.org/:_authToken=${VAR_FOUND}
registry=${VAR_FOUND}
"#,
      &|var_name| match var_name {
        "VAR_FOUND" => Some("SOME_VALUE".to_string()),
        _ => None,
      },
    )
    .unwrap();
    assert_eq!(
      npm_rc,
      NpmRc {
        registry: Some("SOME_VALUE".to_string()),
        scope_registries: HashMap::from([
          ("a".to_string(), "${VAR_FOUND}".to_string()),
          ("myorg".to_string(), "SOME_VALUE".to_string()),
          ("another".to_string(), "${VAR_NOT_FOUND}".to_string()),
        ]),
        registry_configs: HashMap::from([(
          "registry.npmjs.org/".to_string(),
          Arc::new(RegistryConfig {
            auth_token: Some("SOME_VALUE".to_string()),
            ..Default::default()
          })
        ),])
      }
    )
  }

  #[test]
  fn test_expand_vars() {
    assert_eq!(
      expand_vars("test${VAR}test", &|var_name| {
        match var_name {
          "VAR" => Some("VALUE".to_string()),
          _ => None,
        }
      }),
      "testVALUEtest"
    );
    assert_eq!(
      expand_vars("${A}${B}${C}", &|var_name| {
        match var_name {
          "A" => Some("1".to_string()),
          "B" => Some("2".to_string()),
          "C" => Some("3".to_string()),
          _ => None,
        }
      }),
      "123"
    );
    assert_eq!(
      expand_vars("test\\${VAR}test", &|var_name| {
        match var_name {
          "VAR" => Some("VALUE".to_string()),
          _ => None,
        }
      }),
      "test${VAR}test"
    );
    assert_eq!(
      // npm ignores values with $ in them
      expand_vars("test${VA$R}test", &|_| {
        unreachable!();
      }),
      "test${VA$R}test"
    );
    assert_eq!(
      // npm ignores values with { in them
      expand_vars("test${VA{R}test", &|_| {
        unreachable!();
      }),
      "test${VA{R}test"
    );
  }

  #[test]
  fn test_scope_registry_url_only() {
    let npm_rc = NpmRc::parse(
      r#"
@example:registry=https://example.com/
"#,
      &|_| None,
    )
    .unwrap();
    let npm_rc = npm_rc
      .as_resolved(&Url::parse("https://deno.land/npm/").unwrap())
      .unwrap();
    {
      let registry_url = npm_rc.get_registry_url("@example/test");
      let config = npm_rc.get_registry_config("@example/test");
      assert_eq!(registry_url.as_str(), "https://example.com/");
      assert_eq!(config.as_ref(), &RegistryConfig::default());
    }
    {
      let registry_url = npm_rc.get_registry_url("test");
      let config = npm_rc.get_registry_config("test");
      assert_eq!(registry_url.as_str(), "https://deno.land/npm/");
      assert_eq!(config.as_ref(), &Default::default());
    }
  }

  #[test]
  fn test_scope_with_auth() {
    let npm_rc = NpmRc::parse(
      r#"
@example:registry=https://example.com/foo
@example2:registry=https://example2.com/
//example.com/foo/:_authToken=MY_AUTH_TOKEN
; This one is borked - the URL must match registry URL exactly
//example.com2/example/:_authToken=MY_AUTH_TOKEN2
"#,
      &|_| None,
    )
    .unwrap();
    let npm_rc = npm_rc
      .as_resolved(&Url::parse("https://deno.land/npm/").unwrap())
      .unwrap();
    {
      let registry_url = npm_rc.get_registry_url("@example/test");
      let config = npm_rc.get_registry_config("@example/test");
      assert_eq!(registry_url.as_str(), "https://example.com/foo/");
      assert_eq!(
        config.as_ref(),
        &RegistryConfig {
          auth_token: Some("MY_AUTH_TOKEN".to_string()),
          ..Default::default()
        }
      );
    }
    {
      let registry_url = npm_rc.get_registry_url("@example2/test");
      let config = npm_rc.get_registry_config("@example2/test");
      assert_eq!(registry_url.as_str(), "https://example2.com/");
      assert_eq!(config.as_ref(), &Default::default());
    }
  }

  #[test]
  fn test_scope_registry_same_as_env_registry() {
    // a scope registry that matches the env registry url should still
    // be included in the resolved npmrc. This is important because scopes
    // that are overridden by Deno like the @jsr scope might have the registry
    // set to the default registry like this and so we want to ensure it's
    // still used and not overwritten
    let npm_rc = NpmRc::parse(
      r#"
@jsr:registry=https://registry.npmjs.org/
"#,
      &|_| None,
    )
    .unwrap();
    let npm_rc = npm_rc
      .as_resolved(&Url::parse("https://registry.npmjs.org/").unwrap())
      .unwrap();
    assert!(npm_rc.scopes.contains_key("jsr"));
    assert_eq!(
      npm_rc.scopes.get("jsr").unwrap().registry_url.as_str(),
      "https://registry.npmjs.org/"
    );
  }
}
