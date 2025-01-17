// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::convert::Infallible;
use std::sync::atomic::AtomicBool;

use deno_core::convert::OptionNull;
use deno_core::convert::OptionUndefined;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::HandleScope;
use deno_core::v8::Local;
use deno_core::v8::Value;
use deno_core::v8_static_strings;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::GarbageCollected;
use deno_core::ToV8;
use deno_core::WebIDL;
use indexmap::IndexMap;
use urlpattern::quirks;

pub static GROUP_STRING_FALLBACK: AtomicBool = AtomicBool::new(false);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[error(transparent)]
pub enum UrlPatternError {
  #[class(type)]
  #[error("{prefix}: {key} is invalid")]
  InvalidRegexp {
    prefix: &'static str,
    key: &'static str,
  },
  #[class(type)]
  #[error(transparent)]
  UrlPattern(#[from] urlpattern::Error),
  #[class(inherit)]
  #[error(transparent)]
  WebIDL(#[from] WebIdlError),
}

#[derive(WebIDL, Default, Debug)]
#[webidl(dictionary)]
struct URLPatternInit {
  protocol: Option<String>,
  username: Option<String>,
  password: Option<String>,
  hostname: Option<String>,
  port: Option<String>,
  pathname: Option<String>,
  search: Option<String>,
  hash: Option<String>,
  #[webidl(rename = "baseURL")]
  base_url: Option<String>,
}

impl From<URLPatternInit> for quirks::UrlPatternInit {
  fn from(value: URLPatternInit) -> Self {
    Self {
      protocol: value.protocol,
      username: value.username,
      password: value.password,
      hostname: value.hostname,
      port: value.port,
      pathname: value.pathname,
      search: value.search,
      hash: value.hash,
      base_url: value.base_url,
    }
  }
}
impl From<quirks::UrlPatternInit> for URLPatternInit {
  fn from(value: quirks::UrlPatternInit) -> Self {
    Self {
      protocol: value.protocol,
      username: value.username,
      password: value.password,
      hostname: value.hostname,
      port: value.port,
      pathname: value.pathname,
      search: value.search,
      hash: value.hash,
      base_url: value.base_url,
    }
  }
}

#[derive(Debug)]
enum URLPatternInput {
  URLPatternInit(URLPatternInit),
  String(String),
}

impl<'a> WebIdlConverter<'a> for URLPatternInput {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: deno_core::webidl::ContextFn,
    _: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_object() {
      Ok(URLPatternInput::URLPatternInit(URLPatternInit::convert(
        scope,
        value,
        prefix,
        context,
        &Default::default(),
      )?))
    } else {
      Ok(URLPatternInput::String(String::convert(
        scope,
        value,
        prefix,
        context,
        &Default::default(),
      )?))
    }
  }
}

impl From<URLPatternInput> for quirks::StringOrInit {
  fn from(value: URLPatternInput) -> Self {
    match value {
      URLPatternInput::URLPatternInit(init) => {
        quirks::StringOrInit::Init(init.into())
      }
      URLPatternInput::String(s) => quirks::StringOrInit::String(s),
    }
  }
}
impl From<quirks::StringOrInit> for URLPatternInput {
  fn from(value: quirks::StringOrInit) -> Self {
    match value {
      quirks::StringOrInit::Init(init) => {
        URLPatternInput::URLPatternInit(init.into())
      }
      quirks::StringOrInit::String(s) => URLPatternInput::String(s),
    }
  }
}

#[derive(WebIDL, Clone, Debug)]
#[webidl(dictionary)]
struct URLPatternOptions {
  #[webidl(default = false)]
  ignore_case: bool,
}

impl From<URLPatternOptions> for urlpattern::UrlPatternOptions {
  fn from(value: URLPatternOptions) -> Self {
    Self {
      ignore_case: value.ignore_case,
    }
  }
}

pub struct URLPattern {
  pattern: quirks::UrlPattern,

  protocol: v8::Global<v8::RegExp>,
  username: v8::Global<v8::RegExp>,
  password: v8::Global<v8::RegExp>,
  hostname: v8::Global<v8::RegExp>,
  port: v8::Global<v8::RegExp>,
  pathname: v8::Global<v8::RegExp>,
  search: v8::Global<v8::RegExp>,
  hash: v8::Global<v8::RegExp>,
}

impl GarbageCollected for URLPattern {}

#[op2]
impl URLPattern {
  #[constructor]
  #[cppgc]
  fn new<'s>(
    scope: &mut v8::HandleScope<'s>,
    input: v8::Local<'s, v8::Value>,
    base_url_or_options: v8::Local<'s, v8::Value>,
    maybe_options: v8::Local<'s, v8::Value>,
  ) -> Result<URLPattern, UrlPatternError> {
    const PREFIX: &str = "Failed to construct 'URLPattern'";

    let (input, base_url, options) = if base_url_or_options.is_string() {
      // TODO: webidl.requiredArguments(arguments.length, 2, prefix);
      let input = URLPatternInput::convert(
        scope,
        input,
        PREFIX.into(),
        deno_core::webidl::ContextFn::from(|| "Argument 1".into()),
        &Default::default(),
      )?;
      let base_url = Some(String::convert(
        scope,
        base_url_or_options,
        PREFIX.into(),
        deno_core::webidl::ContextFn::from(|| "Argument 2".into()),
        &Default::default(),
      )?);
      let options = URLPatternOptions::convert(
        scope,
        maybe_options,
        PREFIX.into(),
        deno_core::webidl::ContextFn::from(|| "Argument 3".into()),
        &Default::default(),
      )?;
      (input, base_url, options)
    } else {
      let input = if !input.is_undefined() {
        URLPatternInput::convert(
          scope,
          input,
          PREFIX.into(),
          deno_core::webidl::ContextFn::from(|| "Argument 1".into()),
          &Default::default(),
        )?
      } else {
        URLPatternInput::URLPatternInit(URLPatternInit::default())
      };
      let options = URLPatternOptions::convert(
        scope,
        base_url_or_options,
        PREFIX.into(),
        deno_core::webidl::ContextFn::from(|| "Argument 2".into()),
        &Default::default(),
      )?;
      (input, None, options)
    };

    let init = quirks::process_construct_pattern_input(
      input.into(),
      base_url.as_deref(),
    )?;

    let flags = if options.ignore_case {
      v8::RegExpCreationFlags::UNICODE | v8::RegExpCreationFlags::IGNORE_CASE
    } else {
      v8::RegExpCreationFlags::UNICODE
    };

    let pattern = quirks::parse_pattern(init, options.into())?;

    macro_rules! create_regexp_global {
      ($scope:expr, $key:tt, $pattern:expr, $flags:expr) => {{
        let pattern =
          v8::String::new($scope, &$pattern.$key.regexp_string).unwrap();

        let Some(regexp) = v8::RegExp::new($scope, pattern, $flags) else {
          return Err(UrlPatternError::InvalidRegexp {
            prefix: PREFIX,
            key: stringify!($key),
          });
        };
        v8::Global::new($scope, regexp)
      }};
    }

    Ok(URLPattern {
      protocol: create_regexp_global!(scope, protocol, pattern, flags),
      username: create_regexp_global!(scope, username, pattern, flags),
      password: create_regexp_global!(scope, password, pattern, flags),
      hostname: create_regexp_global!(scope, hostname, pattern, flags),
      port: create_regexp_global!(scope, port, pattern, flags),
      pathname: create_regexp_global!(scope, pathname, pattern, flags),
      search: create_regexp_global!(scope, search, pattern, flags),
      hash: create_regexp_global!(scope, hash, pattern, flags),
      pattern,
    })
  }

  #[getter]
  #[string]
  fn protocol(&self) -> String {
    self.pattern.protocol.pattern_string.clone()
  }

  #[getter]
  #[string]
  fn username(&self) -> String {
    self.pattern.username.pattern_string.clone()
  }

  #[getter]
  #[string]
  fn password(&self) -> String {
    self.pattern.password.pattern_string.clone()
  }

  #[getter]
  #[string]
  fn hostname(&self) -> String {
    self.pattern.hostname.pattern_string.clone()
  }

  #[getter]
  #[string]
  fn port(&self) -> String {
    self.pattern.port.pattern_string.clone()
  }

  #[getter]
  #[string]
  fn pathname(&self) -> String {
    self.pattern.pathname.pattern_string.clone()
  }

  #[getter]
  #[string]
  fn search(&self) -> String {
    self.pattern.search.pattern_string.clone()
  }

  #[getter]
  #[string]
  fn hash(&self) -> String {
    self.pattern.hash.pattern_string.clone()
  }

  #[getter]
  fn hasRegExpGroups(&self) -> bool {
    self.pattern.has_regexp_groups
  }

  #[required(1)]
  fn test(
    &self,
    scope: &mut v8::HandleScope,
    #[webidl] input: URLPatternInput,
    #[webidl] base_url: Option<String>,
  ) -> Result<bool, UrlPatternError> {
    let res = quirks::process_match_input(input.into(), base_url.as_deref())?;

    let Some((input, _inputs)) = res else {
      return Ok(false);
    };

    let Some(input) = quirks::parse_match_input(input) else {
      return Ok(false);
    };

    macro_rules! handle_component {
      ($t:tt) => {
        match self.pattern.$t.regexp_string.as_str() {
          "^$" => {
            if input.$t != "" {
              return Ok(false);
            }
          }
          "^(.*)$" => {}
          _ => {
            let subject = v8::String::new(scope, &input.$t).unwrap();
            let regexp = self.$t.open(scope);
            // TODO: handle unwrap
            if regexp.exec(scope, subject).unwrap().is_null() {
              return Ok(false);
            }
          }
        }
      };
    }

    handle_component!(protocol);
    handle_component!(username);
    handle_component!(password);
    handle_component!(hostname);
    handle_component!(port);
    handle_component!(pathname);
    handle_component!(search);
    handle_component!(hash);

    Ok(true)
  }

  #[required(1)]
  #[to_v8]
  fn exec(
    &self,
    scope: &mut v8::HandleScope,
    #[webidl] input: URLPatternInput,
    #[webidl] base_url: Option<String>,
  ) -> Result<OptionNull<URLPatternResult>, UrlPatternError> {
    let res = quirks::process_match_input(input.into(), base_url.as_deref())?;

    let Some((input, original_inputs)) = res else {
      return Ok(None.into());
    };

    let Some(values) = quirks::parse_match_input(input) else {
      return Ok(None.into());
    };

    macro_rules! handle_component {
      ($t:tt) => {{
        let component = &self.pattern.$t;
        let mut result = UrlPatternComponentResult {
          input: values.$t.clone(),
          groups: Default::default(),
        };
        match component.regexp_string.as_str() {
          "^$" => {
            if values.$t != "" {
              return Ok(None.into());
            }
          }
          "^(.*)$" => {
            result.groups.insert("0".to_string(), Some(values.$t));
          }
          _ => {
            let subject = v8::String::new(scope, &values.$t).unwrap();
            let regexp = self.$t.open(scope);
            // TODO: handle unwrap
            let exec_result = regexp.exec(scope, subject).unwrap();
            if exec_result.is_null() {
              return Ok(None.into());
            }
            for i in 0..component.group_name_list.len() {
              // TODO(lucacasonato): this is vulnerable to override mistake
              let res =
                exec_result
                  .get_index(scope, (i as u32) + 1)
                  .and_then(|res| {
                    if res.is_undefined() {
                      None
                    } else {
                      Some(res.to_rust_string_lossy(scope))
                    }
                  });
              let res = if GROUP_STRING_FALLBACK
                .load(std::sync::atomic::Ordering::Relaxed)
              {
                Some(res.unwrap_or_default())
              } else {
                res
              };

              result
                .groups
                .insert(component.group_name_list[i].clone(), res);
            }
          }
        }
        result
      }};
    }

    let mut inputs = vec![original_inputs.0.into()];

    if let Some(original_input) = original_inputs.1 {
      inputs.push(URLPatternInput::String(original_input));
    }

    Ok(
      Some(URLPatternResult {
        inputs,

        protocol: handle_component!(protocol),
        username: handle_component!(username),
        password: handle_component!(password),
        hostname: handle_component!(hostname),
        port: handle_component!(port),
        pathname: handle_component!(pathname),
        search: handle_component!(search),
        hash: handle_component!(hash),
      })
      .into(),
    )
  }
}

struct URLPatternResult {
  inputs: Vec<URLPatternInput>,

  protocol: UrlPatternComponentResult,
  username: UrlPatternComponentResult,
  password: UrlPatternComponentResult,
  hostname: UrlPatternComponentResult,
  port: UrlPatternComponentResult,
  pathname: UrlPatternComponentResult,
  search: UrlPatternComponentResult,
  hash: UrlPatternComponentResult,
}

pub struct UrlPatternComponentResult {
  pub input: String,
  pub groups: IndexMap<String, Option<String>>,
}

v8_static_strings! {
  BASE_URL = "baseURL",
  INPUTS = "inputs",
  PROTOCOL = "protocol",
  USERNAME = "username",
  PASSWORD = "password",
  HOSTNAME = "hostname",
  PORT = "port",
  PATHNAME = "pathname",
  SEARCH = "search",
  HASH = "hash",
  INPUT = "input",
  GROUPS = "groups",
}

impl<'a> ToV8<'a> for URLPatternInit {
  type Error = Infallible;

  fn to_v8(
    self,
    scope: &mut HandleScope<'a>,
  ) -> Result<Local<'a, Value>, Self::Error> {
    let names = vec![
      PROTOCOL.v8_string(scope).unwrap().into(),
      USERNAME.v8_string(scope).unwrap().into(),
      PASSWORD.v8_string(scope).unwrap().into(),
      HOSTNAME.v8_string(scope).unwrap().into(),
      PORT.v8_string(scope).unwrap().into(),
      PATHNAME.v8_string(scope).unwrap().into(),
      SEARCH.v8_string(scope).unwrap().into(),
      HASH.v8_string(scope).unwrap().into(),
      BASE_URL.v8_string(scope).unwrap().into(),
    ];

    let values = vec![
      OptionUndefined::from(self.protocol).to_v8(scope)?,
      OptionUndefined::from(self.username).to_v8(scope)?,
      OptionUndefined::from(self.password).to_v8(scope)?,
      OptionUndefined::from(self.hostname).to_v8(scope)?,
      OptionUndefined::from(self.port).to_v8(scope)?,
      OptionUndefined::from(self.pathname).to_v8(scope)?,
      OptionUndefined::from(self.search).to_v8(scope)?,
      OptionUndefined::from(self.hash).to_v8(scope)?,
      OptionUndefined::from(self.base_url).to_v8(scope)?,
    ];

    let obj = v8::Object::new(scope);

    for (key, val) in names.into_iter().zip(values.into_iter()) {
      obj.set(scope, key, val);
    }

    Ok(obj.into())
  }
}

impl<'a> ToV8<'a> for URLPatternInput {
  type Error = Infallible;

  fn to_v8(
    self,
    scope: &mut HandleScope<'a>,
  ) -> Result<Local<'a, Value>, Self::Error> {
    match self {
      URLPatternInput::URLPatternInit(init) => Ok(init.to_v8(scope)?),
      URLPatternInput::String(s) => {
        Ok(v8::String::new(scope, &s).unwrap().into())
      }
    }
  }
}

impl<'a> ToV8<'a> for URLPatternResult {
  type Error = Infallible;

  fn to_v8(
    self,
    scope: &mut HandleScope<'a>,
  ) -> Result<Local<'a, Value>, Self::Error> {
    let names = vec![
      INPUTS.v8_string(scope).unwrap().into(),
      PROTOCOL.v8_string(scope).unwrap().into(),
      USERNAME.v8_string(scope).unwrap().into(),
      PASSWORD.v8_string(scope).unwrap().into(),
      HOSTNAME.v8_string(scope).unwrap().into(),
      PORT.v8_string(scope).unwrap().into(),
      PATHNAME.v8_string(scope).unwrap().into(),
      SEARCH.v8_string(scope).unwrap().into(),
      HASH.v8_string(scope).unwrap().into(),
    ];

    let inputs = self
      .inputs
      .into_iter()
      .map(|input| input.to_v8(scope))
      .collect::<Result<Vec<_>, _>>()?;

    let inputs = v8::Array::new_with_elements(scope, &inputs);

    let values = vec![
      inputs.into(),
      self.protocol.to_v8(scope)?,
      self.username.to_v8(scope)?,
      self.password.to_v8(scope)?,
      self.hostname.to_v8(scope)?,
      self.port.to_v8(scope)?,
      self.pathname.to_v8(scope)?,
      self.search.to_v8(scope)?,
      self.hash.to_v8(scope)?,
    ];

    let obj = v8::Object::new(scope);

    for (key, val) in names.into_iter().zip(values.into_iter()) {
      obj.set(scope, key, val);
    }

    Ok(obj.into())
  }
}

impl<'a> ToV8<'a> for UrlPatternComponentResult {
  type Error = Infallible;

  fn to_v8(
    self,
    scope: &mut HandleScope<'a>,
  ) -> Result<Local<'a, Value>, Self::Error> {
    let input = self.input.to_v8(scope)?;

    let groups = {
      let len = self.groups.len();
      let mut names = Vec::with_capacity(len);
      let mut values = Vec::with_capacity(len);

      for (key, val) in self.groups {
        names.push(v8::String::new(scope, &key).unwrap().into());
        values.push(OptionUndefined(val).to_v8(scope)?)
      }

      let obj = v8::Object::new(scope);

      for (key, val) in names.into_iter().zip(values.into_iter()) {
        obj.set(scope, key, val);
      }

      obj
    };

    let obj = v8::Object::new(scope);
    let key = INPUT.v8_string(scope).unwrap().into();
    obj.set(scope, key, input);
    let key = GROUPS.v8_string(scope).unwrap().into();
    obj.set(scope, key, groups.into());

    Ok(obj.into())
  }
}
