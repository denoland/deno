// Copyright 2018-2025 the Deno authors. MIT license.

mod urlpattern;

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url;
use deno_core::url::form_urlencoded;
use deno_core::url::quirks;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::v8::HandleScope;
use deno_core::v8::Local;
use deno_core::v8::Value;
use deno_core::webidl;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::GarbageCollected;
use deno_core::JsBuffer;
pub use urlpattern::UrlPatternError;

use crate::urlpattern::op_urlpattern_parse;
use crate::urlpattern::op_urlpattern_process_match_input;

deno_core::extension!(
  deno_url,
  deps = [deno_webidl],
  ops = [
    op_url_parse_search_params,
    op_urlpattern_parse,
    op_urlpattern_process_match_input
  ],
  objects = [URL, URLSearchParams],
  esm = ["00_url.js", "01_urlpattern.js"],
);

#[op2]
#[serde]
pub fn op_url_parse_search_params(
  #[buffer] zero_copy: JsBuffer,
) -> Vec<(String, String)> {
  form_urlencoded::parse(&zero_copy).into_owned().collect()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_url.d.ts")
}

pub struct URL(Rc<RefCell<Url>>);

impl GarbageCollected for URL {}

#[op2]
impl URL {
  #[constructor]
  #[cppgc]
  fn new(
    #[webidl] url: String,
    #[webidl] base: Option<String>,
  ) -> Result<URL, url::ParseError> {
    let base = base.map(|base| Url::parse(&base)).transpose()?;
    let url = Url::options().base_url(base.as_ref()).parse(&url)?;
    Ok(URL(Rc::from(RefCell::new(url))))
  }

  #[static_method]
  #[cppgc]
  fn parse(
    #[webidl] url: String,
    #[webidl] base: Option<String>,
  ) -> Option<URL> {
    let base = base.map(|base| Url::parse(&base)).transpose().ok()?;
    let url = Url::options().base_url(base.as_ref()).parse(&url).ok()?;
    Some(URL(Rc::from(RefCell::new(url))))
  }

  #[static_method]
  fn canParse(#[webidl] url: String, #[webidl] base: Option<String>) -> bool {
    let Ok(base) = base.map(|base| Url::parse(&base)).transpose() else {
      return false;
    };
    Url::options().base_url(base.as_ref()).parse(&url).is_ok()
  }

  #[getter]
  #[string]
  fn hash(&self) -> String {
    quirks::hash(&self.0.borrow()).to_string()
  }

  #[setter]
  fn hash(&self, #[webidl] value: String) {
    quirks::set_hash(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn host(&self) -> String {
    quirks::host(&self.0.borrow()).to_string()
  }

  #[setter]
  fn host(&self, #[webidl] value: String) {
    quirks::set_host(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn hostname(&self) -> String {
    quirks::hostname(&self.0.borrow()).to_string()
  }

  #[setter]
  fn hostname(&self, #[webidl] value: String) {
    quirks::set_hostname(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn href(&self) -> String {
    quirks::href(&self.0.borrow()).to_string()
  }

  #[setter]
  fn href(&self, #[webidl] value: String) {
    quirks::set_href(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn origin(&self) -> String {
    quirks::origin(&self.0.borrow())
  }

  #[getter]
  #[string]
  fn password(&self) -> String {
    quirks::password(&self.0.borrow()).to_string()
  }

  #[setter]
  fn password(&self, #[webidl] value: String) {
    quirks::set_password(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn pathname(&self) -> String {
    quirks::pathname(&self.0.borrow()).to_string()
  }

  #[setter]
  fn pathname(&self, #[webidl] value: String) {
    quirks::set_pathname(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn port(&self) -> String {
    quirks::port(&self.0.borrow()).to_string()
  }

  #[setter]
  fn port(&self, #[webidl] value: String) {
    quirks::set_port(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn protocol(&self) -> String {
    quirks::protocol(&self.0.borrow()).to_string()
  }

  #[setter]
  fn protocol(&self, #[webidl] value: String) {
    quirks::set_protocol(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn search(&self) -> String {
    quirks::search(&self.0.borrow()).to_string()
  }

  #[setter]
  fn search(&self, #[webidl] value: String) {
    quirks::set_search(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[string]
  fn username(&self) -> String {
    quirks::username(&self.0.borrow()).to_string()
  }

  #[setter]
  fn username(&self, #[webidl] value: String) {
    quirks::set_username(&mut self.0.borrow_mut(), &value);
  }

  #[getter]
  #[cppgc]
  fn searchParams(&self) -> URLSearchParams {
    // TODO: sameObject
    let repr =
      form_urlencoded::parse(quirks::search(&self.0.borrow()).as_bytes())
        .into_owned()
        .collect();
    URLSearchParams {
      inner_url: Some(self.0.clone()),
      repr: RefCell::new(repr),
    }
  }

  #[string]
  fn toString(&self) -> String {
    self.0.borrow().to_string()
  }

  #[string]
  fn toJSON(&self) -> String {
    self.0.borrow().to_string()
  }
}

struct URLSearchParams {
  inner_url: Option<Rc<RefCell<Url>>>,
  repr: RefCell<Vec<(String, String)>>,
}
impl GarbageCollected for URLSearchParams {}

#[op2]
impl URLSearchParams {
  // TODO:   constructor(init = "") {
  #[constructor]
  #[cppgc]
  fn new(
    #[webidl] init: SequenceOrRecordOrString,
  ) -> Result<URLSearchParams, AnyError> {
    let repr = match init {
      SequenceOrRecordOrString::Sequence(s) => {
        s.into_iter()
          .enumerate()
          .map(|(i, pair)| {
            if pair.len() != 2 {
              /* TODO:
                throw new TypeError(
                  `${prefix}: Item ${
                    i + 0
                  } in the parameter list does have length 2 exactly`,
                );
              }
                 */
              unreachable!()
            }
            let mut iter = pair.into_iter();
            (iter.next().unwrap(), iter.next().unwrap())
          })
          .collect::<Vec<_>>()
      }
      SequenceOrRecordOrString::Record(r) => r.into_iter().collect(),
      SequenceOrRecordOrString::String(s) => {
        let s = s.strip_prefix('?').unwrap_or(&s);
        form_urlencoded::parse(s.as_bytes()).into_owned().collect()
      }
    };

    Ok(URLSearchParams {
      inner_url: None,
      repr: RefCell::new(repr),
    })
  }

  #[required(2)]
  fn append(&self, #[webidl] name: String, #[webidl] value: String) {
    {
      self.repr.borrow_mut().push((name, value));
    }
    self.update_url();
  }

  #[required(1)]
  fn delete(&self, #[webidl] name: String, #[webidl] value: Option<String>) {
    let mut i = 0;
    if let Some(value) = value {
      let mut list = self.repr.borrow_mut();
      while i < list.len() {
        if list[i].0 == name && list[i].1 == value {
          list.remove(i);
        } else {
          i += 1;
        }
      }
    } else {
      let mut list = self.repr.borrow_mut();
      while i < list.len() {
        if list[i].0 == name {
          list.remove(i);
        } else {
          i += 1;
        }
      }
    }
    self.update_url();
  }

  #[required(1)]
  #[serde]
  fn getAll(&self, #[webidl] name: String) -> Vec<String> {
    self
      .repr
      .borrow()
      .iter()
      .filter_map(|(repr_name, val)| {
        if repr_name == &name {
          Some(val.clone())
        } else {
          None
        }
      })
      .collect()
  }

  #[required(1)]
  #[string]
  fn get(&self, #[webidl] name: String) -> Option<String> {
    self.repr.borrow().iter().find_map(|(repr_name, val)| {
      if repr_name == &name {
        Some(val.clone())
      } else {
        None
      }
    })
  }

  // TODO: dont use option, figure out a solution to differentiate between "= undefined" and nullable converter
  #[required(1)]
  fn has(
    &self,
    #[webidl] name: String,
    #[webidl] value: Option<String>,
  ) -> bool {
    if let Some(value) = value {
      self.repr.borrow().contains(&(name, value))
    } else {
      self
        .repr
        .borrow()
        .iter()
        .any(|(repr_name, _val)| repr_name == &name)
    }
  }

  #[required(2)]
  fn set(&self, #[webidl] name: String, #[webidl] value: String) {
    {
      let mut list = self.repr.borrow_mut();
      let mut i = 0;
      let mut found = false;

      // If there are any name-value pairs whose name is name, in list,
      // set the value of the first such name-value pair to value
      // and remove the others.
      while i < list.len() {
        if list[i].0 == name {
          if !found {
            list[i].1 = value.clone();
            found = true;
            i += 1;
          } else {
            list.remove(i);
          }
        } else {
          i += 1;
        }
      }

      // Otherwise, append a new name-value pair whose name is name
      // and value is value, to list.
      if !found {
        list.push((name, value));
      }
    }

    self.update_url();
  }

  #[fast]
  fn sort(&self) {
    {
      let mut list = self.repr.borrow_mut();
      list.sort_by(|a, b| a.0.cmp(&b.0));
    }

    self.update_url();
  }

  #[string]
  fn toString(&self) -> String {
    self.to_string_repr()
  }

  #[getter]
  fn size(&self) -> u32 {
    self.repr.borrow().len() as _
  }
}

impl URLSearchParams {
  fn update_url(&self) {
    if let Some(inner) = &self.inner_url {
      quirks::set_search(&mut inner.borrow_mut(), &self.to_string_repr());
    }
  }

  fn to_string_repr(&self) -> String {
    let repr = self.repr.borrow();
    form_urlencoded::Serializer::new(String::new())
      .extend_pairs(repr.iter())
      .finish()
  }
}

enum SequenceOrRecordOrString {
  Sequence(Vec<Vec<String>>),
  Record(indexmap::IndexMap<String, String>),
  String(String),
}
impl<'a> WebIdlConverter<'a> for SequenceOrRecordOrString {
  type Options = ();

  fn convert<C>(
    scope: &mut HandleScope<'a>,
    value: Local<'a, Value>,
    prefix: Cow<'static, str>,
    context: C,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError>
  where
    C: Fn() -> Cow<'static, str>,
  {
    if webidl::type_of(scope, value) == webidl::Type::Object {
      let obj = value.try_cast::<v8::Object>().unwrap();
      let iter_key = v8::Symbol::get_iterator(scope);
      if obj.get(scope, iter_key.into()).is_some() {
        Ok(Self::Sequence(WebIdlConverter::convert(
          scope,
          value,
          prefix,
          context,
          &Default::default(),
        )?))
      } else {
        Ok(Self::Record(WebIdlConverter::convert(
          scope,
          value,
          prefix,
          context,
          &Default::default(),
        )?))
      }
    } else {
      Ok(Self::String(WebIdlConverter::convert(
        scope,
        value,
        prefix,
        context,
        &Default::default(),
      )?))
    }
  }
}
