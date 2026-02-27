// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::print_stderr)]

use crate::FastString;
use crate::ModuleCodeString;
use crate::ModuleSource;
use crate::ModuleSpecifier;
use crate::ModuleType;
use crate::ResolutionKind;
use crate::RuntimeOptions;
use crate::ascii_str;
use crate::error::CoreErrorKind;
use crate::error::exception_to_err_result;
use crate::modules::CustomModuleEvaluationKind;
use crate::modules::IntoModuleName;
use crate::modules::ModuleCodeBytes;
use crate::modules::ModuleConcreteError;
use crate::modules::ModuleError;
use crate::modules::ModuleInfo;
use crate::modules::ModuleLoadOptions;
use crate::modules::ModuleRequest;
use crate::modules::ModuleSourceCode;
use crate::modules::RequestedModuleType;
use crate::modules::SourceCodeCacheInfo;
use crate::modules::loaders::ModuleLoadEventCounts;
use crate::modules::loaders::TestingModuleLoader;
use crate::modules::loaders::*;
use crate::resolve_import;
use crate::resolve_url;
use crate::runtime::JsRuntime;
use crate::runtime::JsRuntimeForSnapshot;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_ops::op2;
use futures::future::FutureExt;
use parking_lot::Mutex;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::future::poll_fn;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use tokio::task::LocalSet;
use url::Url;

// deno_ops macros generate code assuming deno_core in scope.
use crate::deno_core;

#[derive(Default)]
struct MockLoader {
  pub loads: Arc<Mutex<Vec<String>>>,
  pub code_cache: HashMap<String, Vec<u8>>,
  pub updated_code_cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockLoader {
  fn new() -> Rc<Self> {
    Default::default()
  }

  fn new_with_code_cache(code_cache: HashMap<String, Vec<u8>>) -> Rc<Self> {
    Rc::new(Self {
      code_cache,
      ..Default::default()
    })
  }
}

fn mock_source_code(url: &str) -> Option<(&'static str, &'static str)> {
  const A_SRC: &str = r#"
import { b } from "/b.js";
import { c } from "/c.js";
if (b() != 'b') throw Error();
if (c() != 'c') throw Error();
if (!import.meta.main) throw Error();
if (import.meta.url != 'file:///a.js') throw Error();
if (import.meta.WasmInstance !== undefined) throw Error();
"#;

  const B_SRC: &str = r#"
import { c } from "/c.js";
if (c() != 'c') throw Error();
export function b() { return 'b'; }
if (import.meta.main) throw Error();
if (import.meta.url != 'file:///b.js') throw Error();
"#;

  const C_SRC: &str = r#"
import { d } from "/d.js";
export function c() { return 'c'; }
if (d() != 'd') throw Error();
if (import.meta.main) throw Error();
if (import.meta.url != 'file:///c.js') throw Error();
"#;

  const D_SRC: &str = r#"
export function d() { return 'd'; }
if (import.meta.main) throw Error();
if (import.meta.url != 'file:///d.js') throw Error();
"#;

  const CIRCULAR1_SRC: &str = r#"
import "/circular2.js";
Deno.core.print("circular1");
"#;

  const CIRCULAR2_SRC: &str = r#"
import "/circular3.js";
Deno.core.print("circular2");
"#;

  const CIRCULAR3_SRC: &str = r#"
import "/circular1.js";
import "/circular2.js";
Deno.core.print("circular3");
"#;

  const REDIRECT1_SRC: &str = r#"
import "./redirect2.js";
Deno.core.print("redirect1");
"#;

  const REDIRECT2_SRC: &str = r#"
import "./redirect3.js";
Deno.core.print("redirect2");
"#;

  const REDIRECT3_SRC: &str = r#"Deno.core.print("redirect3");"#;

  const CONCURRENT_REDIRECT_SRC: &str = r#"
import "./redirect2.js";  // This import loads the module and registers the redirect alias
import "./dir/redirect2.js";  // This import aborts load only after detecting it's an already loaded redirect
import "./dir/redirect3.js";  // Tihs import is still in the queue, preventing RecursiveModuleLoad to transition to LoadState::Done
Deno.core.print("concurrent_redirect");
"#;

  const MAIN_SRC: &str = r#"
// never_ready.js never loads.
import "/never_ready.js";
// slow.js resolves after one tick.
import "/slow.js";
"#;

  const SLOW_SRC: &str = r#"
// Circular import of never_ready.js
// Does this trigger two ModuleLoader calls? It shouldn't.
import "/never_ready.js";
import "/a.js";
"#;

  const BAD_IMPORT_SRC: &str = r#"import "foo";"#;

  // (code, real_module_name)
  let spec: Vec<&str> = url.split("file://").collect();
  match spec[1] {
    "/a.js" => Some((A_SRC, "file:///a.js")),
    "/b.js" => Some((B_SRC, "file:///b.js")),
    "/c.js" => Some((C_SRC, "file:///c.js")),
    "/d.js" => Some((D_SRC, "file:///d.js")),
    "/circular1.js" => Some((CIRCULAR1_SRC, "file:///circular1.js")),
    "/circular2.js" => Some((CIRCULAR2_SRC, "file:///circular2.js")),
    "/circular3.js" => Some((CIRCULAR3_SRC, "file:///circular3.js")),
    "/redirect1.js" => Some((REDIRECT1_SRC, "file:///redirect1.js")),
    "/dir/redirect2.js" => Some((REDIRECT2_SRC, "file:///dir/redirect2.js")),
    // pretend redirect - real module name is different than one requested
    "/redirect2.js" => Some((REDIRECT2_SRC, "file:///dir/redirect2.js")),
    "/dir/redirect3.js" => Some((REDIRECT3_SRC, "file:///redirect3.js")),
    "/concurrent_redirect.js" => {
      Some((CONCURRENT_REDIRECT_SRC, "file:///concurrent_redirect.js"))
    }
    "/slow.js" => Some((SLOW_SRC, "file:///slow.js")),
    "/never_ready.js" => {
      Some(("should never be Ready", "file:///never_ready.js"))
    }
    "/main.js" => Some((MAIN_SRC, "file:///main.js")),
    "/bad_import.js" => Some((BAD_IMPORT_SRC, "file:///bad_import.js")),
    // deliberately empty code.
    "/main_with_code.js" => Some(("", "file:///main_with_code.js")),
    _ => None,
  }
}

#[derive(Debug, PartialEq)]
enum MockError {
  ResolveErr,
  LoadErr,
}

impl fmt::Display for MockError {
  fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
    unimplemented!()
  }
}

impl std::error::Error for MockError {
  fn cause(&self) -> Option<&dyn std::error::Error> {
    unimplemented!()
  }
}

impl JsErrorClass for MockError {
  fn get_class(&self) -> Cow<'static, str> {
    unimplemented!()
  }

  fn get_message(&self) -> Cow<'static, str> {
    unimplemented!()
  }

  fn get_additional_properties(&self) -> deno_error::AdditionalProperties {
    unimplemented!()
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

struct DelayedSourceCodeFuture {
  url: String,
  counter: u32,
  code_cache: Option<Vec<u8>>,
}

impl Future for DelayedSourceCodeFuture {
  type Output = Result<ModuleSource, ModuleLoaderError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    inner.counter += 1;
    if inner.url == "file:///never_ready.js" {
      return Poll::Pending;
    }
    if inner.url == "file:///slow.js" && inner.counter < 2 {
      // TODO(ry) Hopefully in the future we can remove current task
      // notification.
      cx.waker().wake_by_ref();
      return Poll::Pending;
    }
    match mock_source_code(&inner.url) {
      Some(src) => {
        let hash = {
          use std::hash::Hash;
          use std::hash::Hasher;
          let mut hasher = twox_hash::XxHash64::default();
          src.1.hash(&mut hasher);
          hasher.finish()
        };
        Poll::Ready(Ok(ModuleSource::for_test_with_redirect(
          src.0,
          inner.url.as_str(),
          src.1,
          Some(SourceCodeCacheInfo {
            hash,
            data: inner
              .code_cache
              .as_ref()
              .map(|code_cache| Cow::Owned(code_cache.clone())),
          }),
        )))
      }
      None => Poll::Ready(Err(JsErrorBox::from_err(MockError::LoadErr))),
    }
  }
}

impl ModuleLoader for MockLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    let referrer = if referrer == "." {
      "file:///"
    } else {
      referrer
    };

    let output_specifier = match resolve_import(specifier, referrer) {
      Ok(specifier) => specifier,
      Err(..) => {
        return Err(JsErrorBox::from_err(MockError::ResolveErr));
      }
    };

    if mock_source_code(output_specifier.as_ref()).is_some() {
      Ok(output_specifier)
    } else {
      Err(JsErrorBox::from_err(MockError::ResolveErr))
    }
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let mut loads = self.loads.lock();
    loads.push(module_specifier.to_string());
    let url = module_specifier.to_string();
    let code_cache = self.code_cache.get(&url);
    ModuleLoadResponse::Async(
      DelayedSourceCodeFuture {
        url,
        counter: 0,
        code_cache: code_cache.cloned(),
      }
      .boxed(),
    )
  }

  fn code_cache_ready(
    &self,
    module_specifier: ModuleSpecifier,
    _hash: u64,
    code_cache: &[u8],
  ) -> Pin<Box<dyn Future<Output = ()>>> {
    let mut updated_code_cache = self.updated_code_cache.lock();
    updated_code_cache
      .insert(module_specifier.to_string(), code_cache.to_vec());
    async {}.boxed_local()
  }
}

#[test]
fn test_recursive_load() {
  let loader = MockLoader::new();
  let loads = loader.loads.clone();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });
  let spec = resolve_url("file:///a.js").unwrap();
  let a_id_fut = runtime.load_main_es_module(&spec);
  let a_id = futures::executor::block_on(a_id_fut).unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(a_id);
  futures::executor::block_on(runtime.run_event_loop(Default::default()))
    .unwrap();
  let l = loads.lock();
  assert_eq!(
    l.to_vec(),
    vec![
      "file:///a.js",
      "file:///b.js",
      "file:///c.js",
      "file:///d.js"
    ]
  );

  let module_map_rc = runtime.module_map();
  let modules = module_map_rc;

  assert_eq!(
    modules.get_id("file:///a.js", RequestedModuleType::None),
    Some(a_id)
  );
  let b_id = modules
    .get_id("file:///b.js", RequestedModuleType::None)
    .unwrap();
  let c_id = modules
    .get_id("file:///c.js", RequestedModuleType::None)
    .unwrap();
  let d_id = modules
    .get_id("file:///d.js", RequestedModuleType::None)
    .unwrap();
  assert_eq!(
    modules.get_requested_modules(a_id),
    Some(vec![
      ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse("file:///b.js").unwrap(),
          requested_module_type: RequestedModuleType::None,
        },
        specifier_key: Some("/b.js".to_string()),
        referrer_source_offset: Some(19),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      },
      ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse("file:///c.js").unwrap(),
          requested_module_type: RequestedModuleType::None,
        },
        specifier_key: Some("/c.js".to_string()),
        referrer_source_offset: Some(46),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      },
    ])
  );
  assert_eq!(
    modules.get_requested_modules(b_id),
    Some(vec![ModuleRequest {
      reference: crate::modules::ModuleReference {
        specifier: ModuleSpecifier::parse("file:///c.js").unwrap(),
        requested_module_type: RequestedModuleType::None,
      },
      specifier_key: Some("/c.js".to_string()),
      referrer_source_offset: Some(19),
      phase: crate::modules::ModuleImportPhase::Evaluation,
    },])
  );
  assert_eq!(
    modules.get_requested_modules(c_id),
    Some(vec![ModuleRequest {
      reference: crate::modules::ModuleReference {
        specifier: ModuleSpecifier::parse("file:///d.js").unwrap(),
        requested_module_type: RequestedModuleType::None,
      },
      specifier_key: Some("/d.js".to_string()),
      referrer_source_offset: Some(19),
      phase: crate::modules::ModuleImportPhase::Evaluation,
    },])
  );
  assert_eq!(modules.get_requested_modules(d_id), Some(vec![]));
}

#[test]
fn test_mods() {
  let loader = Rc::new(TestingModuleLoader::new(NoopModuleLoader));
  static DISPATCH_COUNT: AtomicUsize = AtomicUsize::new(0);

  #[op2(fast)]
  fn op_test(control: u8) -> u8 {
    DISPATCH_COUNT.fetch_add(1, Ordering::Relaxed);
    assert_eq!(control, 42);
    43
  }

  deno_core::extension!(test_ext, ops = [op_test]);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  runtime
    .execute_script(
      "setup.js",
      r#"
      function assert(cond) {
        if (!cond) {
          throw Error("assert");
        }
      }
      "#,
    )
    .unwrap();

  assert_eq!(DISPATCH_COUNT.load(Ordering::Relaxed), 0);

  let module_map = runtime.module_map().clone();

  let (mod_a, mod_b) = {
    deno_core::scope!(scope, runtime);
    let mod_a = module_map
      .new_es_module(
        scope,
        true,
        ascii_str!("file:///a.js").into(),
        ascii_str!(
          r#"
          import { b } from './b.js'
          if (b() != 'b') throw Error();
          let control = 42;
          Deno.core.ops.op_test(control);
        "#
        )
        .into(),
        false,
        None,
      )
      .unwrap();

    assert_eq!(DISPATCH_COUNT.load(Ordering::Relaxed), 0);
    let imports = module_map.get_requested_modules(mod_a);
    assert_eq!(
      imports,
      Some(vec![ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse("file:///b.js").unwrap(),
          requested_module_type: RequestedModuleType::None,
        },
        specifier_key: Some("./b.js".to_string()),
        referrer_source_offset: Some(29),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      },])
    );

    let mod_b = module_map
      .new_es_module(
        scope,
        false,
        ascii_str!("file:///b.js").into(),
        ascii_str!("export function b() { return 'b' }").into(),
        false,
        None,
      )
      .unwrap();
    let imports = module_map.get_requested_modules(mod_b).unwrap();
    assert_eq!(imports.len(), 0);
    (mod_a, mod_b)
  };

  runtime.instantiate_module(mod_b).unwrap();
  assert_eq!(DISPATCH_COUNT.load(Ordering::Relaxed), 0);
  assert_eq!(loader.counts(), ModuleLoadEventCounts::new(1, 0, 0, 0));

  runtime.instantiate_module(mod_a).unwrap();
  assert_eq!(DISPATCH_COUNT.load(Ordering::Relaxed), 0);

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(mod_a);
  assert_eq!(DISPATCH_COUNT.load(Ordering::Relaxed), 1);
}

#[test]
fn test_lazy_loaded_esm() {
  deno_core::extension!(test_ext, lazy_loaded_esm = [dir "modules/testdata", "lazy_loaded.js"]);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  runtime
    .lazy_load_es_module_with_code(
      "ext:cli/more_lazy.js",
      crate::ascii_str_include!("testdata/lazy_loaded.js"),
    )
    .unwrap();

  runtime
    .execute_script(
      "setup.js",
      r#"
      Deno.core.print("1\n");
      const module = Deno.core.createLazyLoader("ext:test_ext/lazy_loaded.js")();
      module.blah("hello\n");
      Deno.core.print(`${JSON.stringify(module)}\n`);
      const module1 = Deno.core.createLazyLoader("ext:test_ext/lazy_loaded.js")();
      if (module !== module1) throw new Error("should return the same error");
      "#,
    )
    .unwrap();
}

#[test]
fn test_json_text_bytes_modules() {
  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  runtime
    .execute_script(
      "setup.js",
      r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
    )
    .unwrap();

  let module_map = runtime.module_map().clone();

  let (mod_b, mod_c, mod_d, mod_e) = {
    deno_core::scope!(scope, runtime);
    let specifier_b = ascii_str!("file:///b.js");

    let mod_b = module_map
      .new_es_module(
        scope,
        false,
        specifier_b.into(),
        ascii_str!(
          r#"
          import jsonData from './c.json' with {type: "json"};
          assert(jsonData.a == "b");
          assert(jsonData.c.d == 10);
          import txt from './d.txt' with {type: "text"};
          assert(txt == "hello there");
          import bytes from './e.bin' with {type: "bytes"};
          assert(bytes.length === 3);
          assert(bytes[0] === 1);
          assert(bytes[1] === 2);
          assert(bytes[2] === 3);
        "#
        )
        .into(),
        false,
        None,
      )
      .unwrap();

    let imports = module_map.get_requested_modules(mod_b);
    assert_eq!(
      imports,
      Some(vec![
        ModuleRequest {
          reference: crate::modules::ModuleReference {
            specifier: ModuleSpecifier::parse("file:///c.json").unwrap(),
            requested_module_type: RequestedModuleType::Json,
          },
          specifier_key: Some("./c.json".to_string()),
          referrer_source_offset: Some(32),
          phase: crate::modules::ModuleImportPhase::Evaluation,
        },
        ModuleRequest {
          reference: crate::modules::ModuleReference {
            specifier: ModuleSpecifier::parse("file:///d.txt").unwrap(),
            requested_module_type: RequestedModuleType::Text,
          },
          specifier_key: Some("./d.txt".to_string()),
          referrer_source_offset: Some(165),
          phase: crate::modules::ModuleImportPhase::Evaluation,
        },
        ModuleRequest {
          reference: crate::modules::ModuleReference {
            specifier: ModuleSpecifier::parse("file:///e.bin").unwrap(),
            requested_module_type: RequestedModuleType::Bytes,
          },
          specifier_key: Some("./e.bin".to_string()),
          referrer_source_offset: Some(264),
          phase: crate::modules::ModuleImportPhase::Evaluation,
        },
      ])
    );

    let mod_c = module_map
      .new_json_module(
        scope,
        ascii_str!("file:///c.json"),
        ascii_str!("{\"a\": \"b\", \"c\": {\"d\": 10}}"),
      )
      .unwrap();
    assert_eq!(module_map.get_requested_modules(mod_c).unwrap().len(), 0);

    let mod_d = module_map
      .new_text_module(
        scope,
        ascii_str!("file:///d.txt"),
        ascii_str!("hello there"),
      )
      .unwrap();
    assert_eq!(module_map.get_requested_modules(mod_d).unwrap().len(), 0);

    let mod_e = module_map
      .new_bytes_module(
        scope,
        ascii_str!("file:///e.bin"),
        ModuleCodeBytes::Static(&[1, 2, 3]),
      )
      .unwrap();
    assert_eq!(module_map.get_requested_modules(mod_e).unwrap().len(), 0);

    (mod_b, mod_c, mod_d, mod_e)
  };

  runtime.instantiate_module(mod_e).unwrap();
  runtime.instantiate_module(mod_d).unwrap();
  runtime.instantiate_module(mod_c).unwrap();

  assert_eq!(loader.counts(), ModuleLoadEventCounts::new(3, 0, 0, 0));

  runtime.instantiate_module(mod_b).unwrap();

  let receiver = runtime.mod_evaluate(mod_b);
  futures::executor::block_on(runtime.run_event_loop(Default::default()))
    .unwrap();
  futures::executor::block_on(receiver).unwrap();
}

#[test]
fn test_validate_import_attributes_default() {
  // Verify that unless `validate_import_attributes_cb` is passed all import
  // are allowed and don't have any problem executing "invalid" code.

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  let module_map_rc = runtime.module_map().clone();
  deno_core::scope!(scope, runtime);
  module_map_rc
    .new_es_module(
      scope,
      false,
      ascii_str!("file:///a.js").into(),
      ascii_str!(r#"import jsonData from './c.json' with {foo: "bar"};"#)
        .into(),
      false,
      None,
    )
    .unwrap();

  module_map_rc
    .new_es_module(
      scope,
      true,
      ascii_str!("file:///a.js").into(),
      ascii_str!(r#"import jsonData from './c.json' with {type: "bar"};"#)
        .into(),
      false,
      None,
    )
    .unwrap();
}

#[test]
fn test_validate_import_attributes_callback() {
  // Verify that `validate_import_attributes_cb` is called and can deny
  // attributes.

  fn validate_import_attributes(
    scope: &mut v8::PinScope,
    assertions: &HashMap<String, String>,
  ) {
    for (key, value) in assertions {
      let msg = if key != "type" {
        Some(format!("\"{key}\" attribute is not supported."))
      } else if value != "json" {
        Some(format!("\"{value}\" is not a valid module type."))
      } else {
        None
      };

      let Some(msg) = msg else {
        continue;
      };

      let message = v8::String::new(scope, &msg).unwrap();
      let exception = v8::Exception::type_error(scope, message);
      scope.throw_exception(exception);
      return;
    }
  }

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    validate_import_attributes_cb: Some(Box::new(validate_import_attributes)),
    ..Default::default()
  });

  let module_map_rc = runtime.module_map().clone();

  {
    deno_core::scope!(scope, runtime);
    let module_err = module_map_rc
      .new_es_module(
        scope,
        true,
        ascii_str!("file:///a.js").into(),
        ascii_str!(r#"import jsonData from './c.json' with {foo: "bar"};"#)
          .into(),
        false,
        None,
      )
      .unwrap_err();

    let ModuleError::Exception(exc) = module_err else {
      unreachable!();
    };
    let exception = v8::Local::new(scope, exc);
    let err =
      exception_to_err_result::<()>(scope, exception, false, true).unwrap_err();
    assert_eq!(
      err.to_string(),
      "Uncaught TypeError: \"foo\" attribute is not supported."
    );
  }

  {
    deno_core::scope!(scope, runtime);
    let module_err = module_map_rc
      .new_es_module(
        scope,
        true,
        ascii_str!("file:///a.js").into(),
        ascii_str!(r#"import jsonData from './c.json' with {type: "bar"};"#)
          .into(),
        false,
        None,
      )
      .unwrap_err();

    let ModuleError::Exception(exc) = module_err else {
      unreachable!();
    };
    let exception = v8::Local::new(scope, exc);
    let err =
      exception_to_err_result::<()>(scope, exception, false, true).unwrap_err();
    assert_eq!(
      err.to_string(),
      "Uncaught TypeError: \"bar\" is not a valid module type."
    );
  }
}

#[test]
fn test_validate_import_attributes_callback2() {
  fn validate_import_attrs(
    scope: &mut v8::PinScope,
    _attrs: &HashMap<String, String>,
  ) {
    let msg = v8::String::new(scope, "boom!").unwrap();
    let ex = v8::Exception::type_error(scope, msg);
    scope.throw_exception(ex);
  }

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    validate_import_attributes_cb: Some(Box::new(validate_import_attrs)),
    ..Default::default()
  });

  let module_map = runtime.module_map().clone();

  {
    deno_core::scope!(scope, runtime);
    let module_err = module_map
      .new_es_module(
        scope,
        true,
        ascii_str!("file:///a.js").into(),
        ascii_str!(r#"import jsonData from './c.json' with {foo: "bar"};"#)
          .into(),
        false,
        None,
      )
      .unwrap_err();

    let ModuleError::Exception(exc) = module_err else {
      unreachable!();
    };
    let exception = v8::Local::new(scope, exc);
    let err =
      exception_to_err_result::<()>(scope, exception, false, true).unwrap_err();
    assert_eq!(err.to_string(), "Uncaught TypeError: boom!");
  }
}

#[test]
fn test_custom_module_type_default() {
  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  let module_map = runtime.module_map().clone();

  let err = {
    deno_core::scope!(scope, runtime);
    let specifier_a = ascii_str!("file:///a.png").into();
    module_map
      .new_module(
        scope,
        true,
        false,
        ModuleSource {
          code: ModuleSourceCode::Bytes(ModuleCodeBytes::Static(&[])),
          module_url_found: None,
          code_cache: None,
          module_url_specified: specifier_a,
          module_type: ModuleType::Other("bytes".into()),
        },
      )
      .unwrap_err()
  };

  match err {
    ModuleError::Concrete(ModuleConcreteError::UnsupportedKind(kind)) => {
      assert_eq!(kind, "bytes");
    }
    _ => unreachable!(),
  };
}

#[test]
fn test_custom_module_type_callback_synthetic() {
  fn custom_eval_cb(
    scope: &mut v8::PinScope,
    module_type: Cow<'_, str>,
    _module_name: &FastString,
    module_code: ModuleSourceCode,
  ) -> Result<CustomModuleEvaluationKind, JsErrorBox> {
    if module_type != "bar" {
      return Err(JsErrorBox::generic(format!(
        "Can't load '{}' module",
        module_type
      )));
    }

    let buf = match module_code {
      ModuleSourceCode::Bytes(buf) => buf.to_vec(),
      ModuleSourceCode::String(str_) => str_.as_bytes().to_vec(),
    };
    let buf_len: usize = buf.len();
    let backing_store = v8::ArrayBuffer::new_backing_store_from_vec(buf);
    let backing_store_shared = backing_store.make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared);
    let uint8_array = v8::Uint8Array::new(scope, ab, 0, buf_len).unwrap();
    let value: v8::Local<v8::Value> = uint8_array.into();
    let value_global = v8::Global::new(scope, value);
    Ok(CustomModuleEvaluationKind::Synthetic(value_global))
  }

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    custom_module_evaluation_cb: Some(Box::new(custom_eval_cb)),
    ..Default::default()
  });

  let module_map = runtime.module_map().clone();

  let err = {
    deno_core::scope!(scope, runtime);
    let specifier_a = ascii_str!("file:///a.png").into();
    module_map
      .new_module(
        scope,
        true,
        false,
        ModuleSource {
          code: ModuleSourceCode::Bytes(ModuleCodeBytes::Static(&[])),
          module_url_found: None,
          code_cache: None,
          module_url_specified: specifier_a,
          module_type: ModuleType::Other("foo".into()),
        },
      )
      .unwrap_err()
  };

  match err {
    ModuleError::Core(err) => {
      assert_eq!(err.to_string(), "Can't load 'foo' module");
    }
    _ => unreachable!(),
  };

  {
    deno_core::scope!(scope, runtime);
    let specifier_a = ascii_str!("file:///b.png").into();
    module_map
      .new_module(
        scope,
        true,
        false,
        ModuleSource {
          code: ModuleSourceCode::Bytes(ModuleCodeBytes::Static(&[])),
          module_url_found: None,
          code_cache: None,
          module_url_specified: specifier_a,
          module_type: ModuleType::Other("bar".into()),
        },
      )
      .unwrap()
  };
}

#[test]
fn test_custom_module_type_callback_computed() {
  fn custom_eval_cb(
    scope: &mut v8::PinScope,
    module_type: Cow<'_, str>,
    module_name: &FastString,
    module_code: ModuleSourceCode,
  ) -> Result<CustomModuleEvaluationKind, JsErrorBox> {
    if module_type != "foobar" {
      return Err(JsErrorBox::generic(format!(
        "Can't load '{}' module",
        module_type
      )));
    }

    let buf = match module_code {
      ModuleSourceCode::Bytes(buf) => buf.to_vec(),
      ModuleSourceCode::String(str_) => str_.as_bytes().to_vec(),
    };
    let buf_len: usize = buf.len();
    let backing_store = v8::ArrayBuffer::new_backing_store_from_vec(buf);
    let backing_store_shared = backing_store.make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared);
    let uint8_array = v8::Uint8Array::new(scope, ab, 0, buf_len).unwrap();
    let value: v8::Local<v8::Value> = uint8_array.into();
    let value_global = v8::Global::new(scope, value);

    let computed_src = format!(
      r#"
import bytes from "{}" with {{ type: "foobar-synth" }};

export const foo = bytes;
    "#,
      module_name.as_str()
    );
    Ok(CustomModuleEvaluationKind::ComputedAndSynthetic(
      computed_src.into(),
      value_global,
      ModuleType::Other("foobar-synth".into()),
    ))
  }

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    custom_module_evaluation_cb: Some(Box::new(custom_eval_cb)),
    ..Default::default()
  });

  let module_map = runtime.module_map().clone();

  let mod_id = {
    deno_core::scope!(scope, runtime);
    let specifier_a = ascii_str!("file:///b.png").into();
    module_map
      .new_module(
        scope,
        true,
        false,
        ModuleSource {
          code: ModuleSourceCode::Bytes(ModuleCodeBytes::Static(&[])),
          module_url_found: None,
          code_cache: None,
          module_url_specified: specifier_a,
          module_type: ModuleType::Other("foobar".into()),
        },
      )
      .unwrap()
  };

  let data = module_map.get_data();
  let data = data.borrow();
  let info = data.info.get(mod_id).unwrap();
  assert_eq!(
    info,
    &ModuleInfo {
      id: mod_id,
      main: true,
      name: "file:///b.png".into_module_name(),
      requests: vec![ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse("file:///b.png").unwrap(),
          requested_module_type: RequestedModuleType::Other(
            "foobar-synth".into()
          ),
        },
        specifier_key: Some("file:///b.png".to_string()),
        referrer_source_offset: Some(19),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      }],
      module_type: ModuleType::Other("foobar".into()),
    }
  );
  let info = data.info.get(mod_id - 1).unwrap();
  assert_eq!(
    info,
    &ModuleInfo {
      id: mod_id - 1,
      main: false,
      name: "file:///b.png".into_module_name(),
      requests: vec![],
      module_type: ModuleType::Other("foobar-synth".into()),
    }
  );
}

#[tokio::test]
async fn dyn_import_err() {
  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  // Test an erroneous dynamic import where the specified module isn't found.
  poll_fn(move |cx| {
    runtime
      .execute_script(
        "file:///dyn_import2.js",
        r#"
      (async () => {
        await import("/foo.js");
      })();
      "#,
      )
      .unwrap();

    // We should get an error here.
    let result = runtime.poll_event_loop(cx, Default::default());
    assert!(matches!(result, Poll::Ready(Err(_))));
    assert_eq!(loader.counts(), ModuleLoadEventCounts::new(4, 1, 1, 1));
    Poll::Ready(())
  })
  .await;
}

#[tokio::test]
async fn dyn_import_recurse_err() {
  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::default()));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  runtime
    .execute_script(
      "file:///dyn_import2.js",
      r#"
function bar(v) {
  bar(import(0));
}
bar("foo");
      "#,
    )
    .expect_err("should throw range error");
}

#[tokio::test]
async fn dyn_import_ok() {
  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::with(
    Url::parse("file:///b.js").unwrap(),
    ascii_str!("export function b() { return 'b' }"),
  )));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });
  poll_fn(move |cx| {
    // Dynamically import mod_b
    runtime
      .execute_script(
        "file:///dyn_import3.js",
        r#"
        (async () => {
          let mod = await import("./b.js");
          if (mod.b() !== 'b') {
            throw Error("bad1");
          }
          // And again!
          mod = await import("./b.js");
          if (mod.b() !== 'b') {
            throw Error("bad2");
          }
        })();
        "#,
      )
      .unwrap();

    assert!(matches!(
      runtime.poll_event_loop(cx, Default::default()),
      Poll::Ready(Ok(_))
    ));
    assert_eq!(loader.counts(), ModuleLoadEventCounts::new(5, 1, 1, 1));
    assert!(matches!(
      runtime.poll_event_loop(cx, Default::default()),
      Poll::Ready(Ok(_))
    ));
    assert_eq!(loader.counts(), ModuleLoadEventCounts::new(5, 1, 1, 1));
    Poll::Ready(())
  })
  .await;
}

#[tokio::test]
async fn dyn_import_borrow_mut_error() {
  // https://github.com/denoland/deno/issues/6054
  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::with(
    Url::parse("file:///b.js").unwrap(),
    ascii_str!("export function b() { return 'b' }"),
  )));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  poll_fn(move |cx| {
    runtime
      .execute_script(
        "file:///dyn_import3.js",
        r#"
        (async () => {
          let mod = await import("./b.js");
          if (mod.b() !== 'b') {
            throw Error("bad");
          }
        })();
        "#,
      )
      .unwrap();
    // TODO(mmastrac): These checks here are pretty broken and make a lot of assumptions
    // about the ordering of resolution and events. If these break, we can remove them.

    // Old comments that are likely wrong:
    // First poll runs `prepare_load` hook.
    let _ = runtime.poll_event_loop(cx, Default::default());
    assert_eq!(loader.counts(), ModuleLoadEventCounts::new(4, 1, 1, 1));
    // Second poll triggers error
    let _ = runtime.poll_event_loop(cx, Default::default());
    assert_eq!(loader.counts(), ModuleLoadEventCounts::new(4, 1, 1, 1));
    Poll::Ready(())
  })
  .await;
}

#[test]
fn test_circular_load() {
  let loader = MockLoader::new();
  let loads = loader.loads.clone();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let fut = async move {
    let spec = resolve_url("file:///circular1.js").unwrap();
    let result = runtime.load_main_es_module(&spec).await;
    assert!(result.is_ok());
    let circular1_id = result.unwrap();
    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(circular1_id);
    runtime.run_event_loop(Default::default()).await.unwrap();

    let l = loads.lock();
    assert_eq!(
      l.to_vec(),
      vec![
        "file:///circular1.js",
        "file:///circular2.js",
        "file:///circular3.js"
      ]
    );

    let module_map_rc = runtime.module_map();
    let modules = module_map_rc;

    assert_eq!(
      modules.get_id("file:///circular1.js", RequestedModuleType::None),
      Some(circular1_id)
    );
    let circular2_id = modules
      .get_id("file:///circular2.js", RequestedModuleType::None)
      .unwrap();

    assert_eq!(
      modules.get_requested_modules(circular1_id),
      Some(vec![ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse("file:///circular2.js").unwrap(),
          requested_module_type: RequestedModuleType::None,
        },
        specifier_key: Some("/circular2.js".to_string()),
        referrer_source_offset: Some(8),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      }])
    );

    assert_eq!(
      modules.get_requested_modules(circular2_id),
      Some(vec![ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse("file:///circular3.js").unwrap(),
          requested_module_type: RequestedModuleType::None,
        },
        specifier_key: Some("/circular3.js".to_string()),
        referrer_source_offset: Some(8),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      }])
    );

    assert!(
      modules
        .get_id("file:///circular3.js", RequestedModuleType::None)
        .is_some()
    );
    let circular3_id = modules
      .get_id("file:///circular3.js", RequestedModuleType::None)
      .unwrap();
    assert_eq!(
      modules.get_requested_modules(circular3_id),
      Some(vec![
        ModuleRequest {
          reference: crate::modules::ModuleReference {
            specifier: ModuleSpecifier::parse("file:///circular1.js").unwrap(),
            requested_module_type: RequestedModuleType::None,
          },
          specifier_key: Some("/circular1.js".to_string()),
          referrer_source_offset: Some(8),
          phase: crate::modules::ModuleImportPhase::Evaluation,
        },
        ModuleRequest {
          reference: crate::modules::ModuleReference {
            specifier: ModuleSpecifier::parse("file:///circular2.js").unwrap(),
            requested_module_type: RequestedModuleType::None,
          },
          specifier_key: Some("/circular2.js".to_string()),
          referrer_source_offset: Some(32),
          phase: crate::modules::ModuleImportPhase::Evaluation,
        }
      ])
    );
  }
  .boxed_local();

  futures::executor::block_on(fut);
}

#[test]
fn test_redirect_load() {
  let loader = MockLoader::new();
  let loads = loader.loads.clone();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let fut = async move {
    let spec = resolve_url("file:///redirect1.js").unwrap();
    let result = runtime.load_main_es_module(&spec).await;
    assert!(result.is_ok());
    let redirect1_id = result.unwrap();
    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(redirect1_id);
    runtime.run_event_loop(Default::default()).await.unwrap();
    let l = loads.lock();
    assert_eq!(
      l.to_vec(),
      vec![
        "file:///redirect1.js",
        "file:///redirect2.js",
        "file:///dir/redirect3.js"
      ]
    );

    let module_map_rc = runtime.module_map();
    let modules = module_map_rc;

    assert_eq!(
      modules.get_id("file:///redirect1.js", RequestedModuleType::None),
      Some(redirect1_id)
    );

    let redirect2_id = modules
      .get_id("file:///dir/redirect2.js", RequestedModuleType::None)
      .unwrap();
    assert!(
      modules.is_alias("file:///redirect2.js", RequestedModuleType::None)
    );
    assert!(
      !modules.is_alias("file:///dir/redirect2.js", RequestedModuleType::None)
    );
    assert_eq!(
      modules.get_id("file:///redirect2.js", RequestedModuleType::None),
      Some(redirect2_id)
    );

    let redirect3_id = modules
      .get_id("file:///redirect3.js", RequestedModuleType::None)
      .unwrap();
    assert!(
      modules.is_alias("file:///dir/redirect3.js", RequestedModuleType::None)
    );
    assert!(
      !modules.is_alias("file:///redirect3.js", RequestedModuleType::None)
    );
    assert_eq!(
      modules.get_id("file:///dir/redirect3.js", RequestedModuleType::None),
      Some(redirect3_id)
    );
  }
  .boxed_local();

  futures::executor::block_on(fut);
}

#[test]
fn test_concurrent_redirect_load() {
  let loader = MockLoader::new();
  let loads = loader.loads.clone();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let fut = async move {
    let spec = resolve_url("file:///concurrent_redirect.js").unwrap();
    let result = runtime.load_main_es_module(&spec).await;
    assert!(result.is_ok());
    let concurrent_redirect = result.unwrap();
    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(concurrent_redirect);
    runtime.run_event_loop(Default::default()).await.unwrap();
    let l = loads.lock();
    assert_eq!(
      l.to_vec(),
      vec![
        "file:///concurrent_redirect.js",
        "file:///redirect2.js",
        "file:///dir/redirect3.js"
      ]
    );

    let module_map_rc = runtime.module_map();
    let modules = module_map_rc;

    assert_eq!(
      modules.get_id("file:///redirect2.js", RequestedModuleType::None),
      modules.get_id("file:///dir/redirect2.js", RequestedModuleType::None)
    );
    assert!(
      modules.is_alias("file:///redirect2.js", RequestedModuleType::None)
    );
    assert!(
      !modules.is_alias("file:///dir/redirect2.js", RequestedModuleType::None)
    );
  }
  .boxed_local();

  futures::executor::block_on(fut);
}

#[tokio::test]
async fn slow_never_ready_modules() {
  let loader = MockLoader::new();
  let loads = loader.loads.clone();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  poll_fn(move |cx| {
    let spec = resolve_url("file:///main.js").unwrap();
    let mut recursive_load = runtime.load_main_es_module(&spec).boxed_local();

    let result = recursive_load.poll_unpin(cx);
    assert!(result.is_pending());

    // TODO(ry) Arguably the first time we poll only the following modules
    // should be loaded:
    //      "file:///main.js",
    //      "file:///never_ready.js",
    //      "file:///slow.js"
    // But due to current task notification in DelayedSourceCodeFuture they
    // all get loaded in a single poll.

    for _ in 0..10 {
      let result = recursive_load.poll_unpin(cx);
      assert!(result.is_pending());
      let l = loads.lock();
      assert_eq!(
        l.to_vec(),
        vec![
          "file:///main.js",
          "file:///never_ready.js",
          "file:///slow.js",
          "file:///a.js",
          "file:///b.js",
          "file:///c.js",
          "file:///d.js"
        ]
      );
    }
    Poll::Ready(())
  })
  .await;
}

#[tokio::test]
async fn loader_disappears_after_error() {
  let loader = MockLoader::new();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let spec = resolve_url("file:///bad_import.js").unwrap();
  let result = runtime.load_main_es_module(&spec).await;

  let CoreErrorKind::JsBox(err) = result.unwrap_err().into_kind() else {
    unreachable!();
  };
  assert_eq!(
    err.get_ref().downcast_ref::<MockError>().unwrap(),
    &MockError::ResolveErr
  );
}

#[test]
fn recursive_load_main_with_code() {
  const MAIN_WITH_CODE_SRC: &str = r#"
    import { b } from "/b.js";
    import { c } from "/c.js";
    if (b() != 'b') throw Error();
    if (c() != 'c') throw Error();
    if (!import.meta.main) throw Error();
    if (import.meta.url != 'file:///main_with_code.js') throw Error();
  "#;

  let loader = MockLoader::new();
  let loads = loader.loads.clone();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });
  // In default resolution code should be empty.
  // Instead we explicitly pass in our own code.
  // The behavior should be very similar to /a.js.
  let spec = resolve_url("file:///main_with_code.js").unwrap();
  let main_id_fut = runtime
    .load_main_es_module_from_code(&spec, MAIN_WITH_CODE_SRC)
    .boxed_local();
  let main_id = futures::executor::block_on(main_id_fut).unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(main_id);
  futures::executor::block_on(runtime.run_event_loop(Default::default()))
    .unwrap();

  let l = loads.lock();
  assert_eq!(
    l.to_vec(),
    vec!["file:///b.js", "file:///c.js", "file:///d.js"]
  );

  let module_map_rc = runtime.module_map();
  let modules = module_map_rc;

  assert_eq!(
    modules.get_id("file:///main_with_code.js", RequestedModuleType::None),
    Some(main_id)
  );
  let b_id = modules
    .get_id("file:///b.js", RequestedModuleType::None)
    .unwrap();
  let c_id = modules
    .get_id("file:///c.js", RequestedModuleType::None)
    .unwrap();
  let d_id = modules
    .get_id("file:///d.js", RequestedModuleType::None)
    .unwrap();

  assert_eq!(
    modules.get_requested_modules(main_id),
    Some(vec![
      ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse("file:///b.js").unwrap(),
          requested_module_type: RequestedModuleType::None,
        },
        specifier_key: Some("/b.js".to_string()),
        referrer_source_offset: Some(23),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      },
      ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse("file:///c.js").unwrap(),
          requested_module_type: RequestedModuleType::None,
        },
        specifier_key: Some("/c.js".to_string()),
        referrer_source_offset: Some(54),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      }
    ])
  );
  assert_eq!(
    modules.get_requested_modules(b_id),
    Some(vec![ModuleRequest {
      reference: crate::modules::ModuleReference {
        specifier: ModuleSpecifier::parse("file:///c.js").unwrap(),
        requested_module_type: RequestedModuleType::None,
      },
      specifier_key: Some("/c.js".to_string()),
      referrer_source_offset: Some(19),
      phase: crate::modules::ModuleImportPhase::Evaluation,
    }])
  );
  assert_eq!(
    modules.get_requested_modules(c_id),
    Some(vec![ModuleRequest {
      reference: crate::modules::ModuleReference {
        specifier: ModuleSpecifier::parse("file:///d.js").unwrap(),
        requested_module_type: RequestedModuleType::None,
      },
      specifier_key: Some("/d.js".to_string()),
      referrer_source_offset: Some(19),
      phase: crate::modules::ModuleImportPhase::Evaluation,
    }])
  );
  assert_eq!(modules.get_requested_modules(d_id), Some(vec![]));
}

#[test]
fn main_and_side_module() {
  let main_specifier = resolve_url("file:///main_module.js").unwrap();
  let side_specifier = resolve_url("file:///side_module.js").unwrap();

  let loader = StaticModuleLoader::new([
    (
      main_specifier.clone(),
      ascii_str!("if (!import.meta.main) throw Error();"),
    ),
    (
      side_specifier.clone(),
      ascii_str!("if (import.meta.main) throw Error();"),
    ),
  ]);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(loader)),
    ..Default::default()
  });

  let main_id_fut = runtime.load_main_es_module(&main_specifier).boxed_local();
  let main_id = futures::executor::block_on(main_id_fut).unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(main_id);
  futures::executor::block_on(runtime.run_event_loop(Default::default()))
    .unwrap();

  // Try to add another main module - it should error.
  let side_id_fut = runtime.load_main_es_module(&side_specifier).boxed_local();
  futures::executor::block_on(side_id_fut).unwrap_err();

  // And now try to load it as a side module
  let side_id_fut = runtime.load_side_es_module(&side_specifier).boxed_local();
  let side_id = futures::executor::block_on(side_id_fut).unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(side_id);
  futures::executor::block_on(runtime.run_event_loop(Default::default()))
    .unwrap();
}

#[test]
fn dynamic_imports_snapshot() {
  //TODO: Once the issue with the ModuleNamespaceEntryGetter is fixed, we can maintain a reference to the module
  // and use it when loading the snapshot
  let snapshot = {
    const MAIN_WITH_CODE_SRC: &str = r#"
      await import("./b.js");
    "#;

    let loader = MockLoader::new();
    let mut runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    // In default resolution code should be empty.
    // Instead we explicitly pass in our own code.
    // The behavior should be very similar to /a.js.
    let spec = resolve_url("file:///main_with_code.js").unwrap();
    let main_id_fut = runtime
      .load_main_es_module_from_code(&spec, MAIN_WITH_CODE_SRC)
      .boxed_local();
    let main_id = futures::executor::block_on(main_id_fut).unwrap();

    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(main_id);
    futures::executor::block_on(runtime.run_event_loop(Default::default()))
      .unwrap();
    runtime.snapshot()
  };

  let snapshot = Box::leak(snapshot);
  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });

  //Evaluate the snapshot with an empty function
  runtime2.execute_script("check.js", "true").unwrap();
}

#[test]
fn import_meta_snapshot() {
  let snapshot = {
    const MAIN_WITH_CODE_SRC: &str = r#"
      if (import.meta.url != 'file:///main_with_code.js') throw Error();
      globalThis.meta = import.meta;
      globalThis.url = import.meta.url;
    "#;

    let loader = MockLoader::new();
    let mut runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    // In default resolution code should be empty.
    // Instead we explicitly pass in our own code.
    // The behavior should be very similar to /a.js.
    let spec = resolve_url("file:///main_with_code.js").unwrap();
    let main_id_fut = runtime
      .load_main_es_module_from_code(&spec, MAIN_WITH_CODE_SRC)
      .boxed_local();
    let main_id = futures::executor::block_on(main_id_fut).unwrap();

    #[allow(clippy::let_underscore_future)]
    let eval_fut = runtime.mod_evaluate(main_id);
    futures::executor::block_on(runtime.run_event_loop(Default::default()))
      .unwrap();
    futures::executor::block_on(eval_fut).unwrap();
    runtime.snapshot()
  };

  let snapshot = Box::leak(snapshot);
  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });

  runtime2
    .execute_script(
      "check.js",
      "if (globalThis.url !== 'file:///main_with_code.js') throw Error('x')",
    )
    .unwrap();
}

#[tokio::test]
async fn no_duplicate_loads() {
  // Both of these imports will cause redirects - ie. their "found" specifier
  // will be different than requested specifier.
  const MAIN_SRC: &str = r#"
  import "https://example.com/foo.js";
  import "https://example.com/bar.js";
  "#;

  // foo.js is importing bar.js that is already redirected
  const FOO_SRC: &str = r#"
  import "https://example.com/v1/bar.js";
  "#;

  struct ConsumingLoader {
    pub files: Rc<RefCell<HashMap<String, Option<String>>>>,
  }

  impl Default for ConsumingLoader {
    fn default() -> Self {
      let files = HashMap::from_iter([
        (
          "https://example.com/v1/foo.js".to_string(),
          Some(FOO_SRC.to_string()),
        ),
        (
          "https://example.com/v1/bar.js".to_string(),
          Some("".to_string()),
        ),
        ("file:///main.js".to_string(), Some(MAIN_SRC.to_string())),
      ]);
      Self {
        files: Rc::new(RefCell::new(files)),
      }
    }
  }

  impl ModuleLoader for ConsumingLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
      let referrer = if referrer == "." {
        "file:///"
      } else {
        referrer
      };

      resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
    }

    fn load(
      &self,
      module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<&ModuleLoadReferrer>,
      _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
      let found_specifier =
        if module_specifier.as_str() == "https://example.com/foo.js" {
          Some("https://example.com/v1/foo.js".to_string())
        } else if module_specifier.as_str() == "https://example.com/bar.js" {
          Some("https://example.com/v1/bar.js".to_string())
        } else {
          None
        };

      let mut files = self.files.borrow_mut();
      eprintln!(
        "getting specifier {} {:?}",
        module_specifier.as_str(),
        found_specifier
      );
      let source_code = if let Some(found) = &found_specifier {
        files.get_mut(found).unwrap().take().unwrap()
      } else {
        files
          .get_mut(module_specifier.as_str())
          .unwrap()
          .take()
          .unwrap()
      };
      let module_source = ModuleSource {
        code: ModuleSourceCode::String(ModuleCodeString::from(source_code)),
        module_type: ModuleType::JavaScript,
        code_cache: None,
        module_url_specified: module_specifier.clone().into(),
        module_url_found: found_specifier.map(|s| s.into()),
      };
      ModuleLoadResponse::Sync(Ok(module_source))
    }
  }

  let loader = Rc::new(ConsumingLoader::default());
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let spec = resolve_url("file:///main.js").unwrap();
  let a_id = runtime.load_main_es_module(&spec).await.unwrap();
  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(a_id);
  runtime.run_event_loop(Default::default()).await.unwrap();
}

#[tokio::test]
async fn import_meta_resolve() {
  struct Loader;

  impl ModuleLoader for Loader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
      let referrer = if referrer == "." {
        "file:///"
      } else {
        referrer
      };
      resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
    }

    fn import_meta_resolve(
      &self,
      specifier: &str,
      _referrer: &str,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
      if specifier == "foo" {
        return Ok(ModuleSpecifier::parse("foo:bar").unwrap());
      }

      if specifier == "./mod.js" {
        return Ok(ModuleSpecifier::parse("file:///mod.js").unwrap());
      }

      Err(JsErrorBox::generic("unexpected"))
    }

    fn load(
      &self,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<&ModuleLoadReferrer>,
      _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
      unreachable!();
    }
  }

  let loader = Rc::new(Loader);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let spec = ModuleSpecifier::parse("file:///test.js").unwrap();
  let source = r#"
    if (import.meta.resolve("foo") !== "foo:bar") throw new Error("a");
    if (import.meta.resolve("./mod.js") !== "file:///mod.js") throw new Error("b");
    let caught = false;
    try {
      import.meta.resolve("boom!");
    } catch (e) {
      if (!(e instanceof Error)) throw new Error("c");
      caught = true;
    }
    if (!caught) throw new Error("d");
  "#;

  let a_id = runtime
    .load_main_es_module_from_code(&spec, source)
    .await
    .unwrap();
  let local = LocalSet::new();
  let a = local.spawn_local(runtime.mod_evaluate(a_id));
  let b = local.spawn_local(async move {
    runtime.run_event_loop(Default::default()).await
  });
  local.await;

  a.await.unwrap().unwrap();
  b.await.unwrap().unwrap();
}

#[test]
fn builtin_core_module() {
  let main_specifier = resolve_url("ext:///main_module.js").unwrap();

  let source_code = r#"
    import { core, primordials, internals } from "ext:core/mod.js";
    if (typeof core === "undefined") throw new Error("core missing");
    if (typeof primordials === "undefined") throw new Error("core missing");
    if (typeof internals === "undefined") throw new Error("core missing");
  "#;
  let loader = StaticModuleLoader::new([(main_specifier.clone(), source_code)]);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(loader)),
    ..Default::default()
  });

  let main_id_fut = runtime.load_main_es_module(&main_specifier).boxed_local();
  let main_id = futures::executor::block_on(main_id_fut).unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(main_id);
  futures::executor::block_on(runtime.run_event_loop(Default::default()))
    .unwrap();
}

#[test]
fn import_meta_filename_dirname() {
  #[cfg(not(target_os = "windows"))]
  let main_specifier = resolve_url("file:///main_module.js").unwrap();
  #[cfg(not(target_os = "windows"))]
  let code = r#"if (import.meta.filename != '/main_module.js') throw Error();
    if (import.meta.dirname != '/') throw Error();
  "#;

  #[cfg(target_os = "windows")]
  let main_specifier = resolve_url("file:///C:/main_module.js").unwrap();
  #[cfg(target_os = "windows")]
  let code = r#"if (import.meta.filename != 'C:\\main_module.js') throw Error();
    if (import.meta.dirname != 'C:\\') throw Error();
  "#;

  let loader = StaticModuleLoader::new([(main_specifier.clone(), code)]);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(loader)),
    ..Default::default()
  });

  let main_id_fut = runtime.load_main_es_module(&main_specifier).boxed_local();
  let main_id = futures::executor::block_on(main_id_fut).unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(main_id);
  futures::executor::block_on(runtime.run_event_loop(Default::default()))
    .unwrap();
}

#[test]
fn test_load_with_code_cache() {
  let mut code_cache = {
    let loader = MockLoader::new();
    let loads = loader.loads.clone();
    let updated_code_cache = loader.updated_code_cache.clone();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    let spec = resolve_url("file:///a.js").unwrap();
    let a_id_fut = runtime.load_main_es_module(&spec);
    let a_id = futures::executor::block_on(a_id_fut).unwrap();

    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(a_id);
    futures::executor::block_on(runtime.run_event_loop(Default::default()))
      .unwrap();
    let l = loads.lock();
    assert_eq!(
      l.to_vec(),
      vec![
        "file:///a.js",
        "file:///b.js",
        "file:///c.js",
        "file:///d.js"
      ]
    );

    let c = updated_code_cache.lock();
    let mut keys = c.keys().collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
      keys,
      vec![
        "file:///a.js",
        "file:///b.js",
        "file:///c.js",
        "file:///d.js"
      ]
    );
    c.clone()
  };

  {
    // Create another runtime and try to use the code cache.
    let loader = MockLoader::new_with_code_cache(code_cache.clone());
    let loads = loader.loads.clone();
    let updated_code_cache = loader.updated_code_cache.clone();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    let spec = resolve_url("file:///a.js").unwrap();
    let a_id_fut = runtime.load_main_es_module(&spec);
    let a_id = futures::executor::block_on(a_id_fut).unwrap();

    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(a_id);
    futures::executor::block_on(runtime.run_event_loop(Default::default()))
      .unwrap();
    let l = loads.lock();
    assert_eq!(
      l.to_vec(),
      vec![
        "file:///a.js",
        "file:///b.js",
        "file:///c.js",
        "file:///d.js"
      ]
    );

    // Verify that code cache was not updated, which means that provided code cache was used.
    let c = updated_code_cache.lock();
    assert!(c.is_empty());
  }

  {
    // Create another runtime, and only use code cache for c.js and d.js.
    code_cache.remove("file:///a.js");
    code_cache.remove("file:///b.js");

    let loader = MockLoader::new_with_code_cache(code_cache);
    let loads = loader.loads.clone();
    let updated_code_cache = loader.updated_code_cache.clone();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    let spec = resolve_url("file:///a.js").unwrap();
    let a_id_fut = runtime.load_main_es_module(&spec);
    let a_id = futures::executor::block_on(a_id_fut).unwrap();

    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(a_id);
    futures::executor::block_on(runtime.run_event_loop(Default::default()))
      .unwrap();
    let l = loads.lock();
    assert_eq!(
      l.to_vec(),
      vec![
        "file:///a.js",
        "file:///b.js",
        "file:///c.js",
        "file:///d.js"
      ]
    );

    // Verify that we only used code cache for c.js and d.js, and created new cache for a.js and b.js.
    let c = updated_code_cache.lock();
    let mut keys = c.keys().collect::<Vec<_>>();
    keys.sort();
    assert_eq!(keys, vec!["file:///a.js", "file:///b.js",]);
  }
}

#[test]
fn ext_module_loader_relative() {
  let loader = ExtModuleLoader::new(vec![], None);
  let cases = [
    (
      ("../../foo.js", "ext:test/nested/mod/bar.js"),
      "ext:test/foo.js",
    ),
    (("./foo.js", "ext:test/bar.js"), "ext:test/foo.js"),
    ((".././foo.js", "ext:test/bar.js"), "ext:foo.js"),
    (("./foo.js", "ext:bar.js"), "ext:foo.js"),
  ];
  for ((specifier, referrer), expected) in cases {
    let result = loader
      .resolve(specifier, referrer, ResolutionKind::Import)
      .unwrap();
    assert_eq!(result.as_str(), expected);
  }
}

#[test]
fn invalid_utf8_module() {
  let get_string_source = ModuleSource::get_string_source(
    ModuleSourceCode::Bytes(ModuleCodeBytes::Static(b"// \xFE\xFE\xFF\xFF")),
  );
  assert_eq!(
    get_string_source,
    FastString::from_static("// \u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}")
  );
}

#[tokio::test]
async fn evaluate_already_evaluated_module() {
  // This test verifies that calling mod_evaluate on an already-evaluated module
  // doesn't panic, but instead returns Ok(()) immediately. This can happen when
  // the same module is specified both as a preload module and as the main module.

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::with(
    Url::parse("file:///main.js").unwrap(),
    ascii_str!(
      "globalThis.executionCount = (globalThis.executionCount || 0) + 1;"
    ),
  )));

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  let spec = resolve_url("file:///main.js").unwrap();

  // Load and evaluate the module for the first time
  let mod_id = runtime.load_main_es_module(&spec).await.unwrap();
  let receiver = runtime.mod_evaluate(mod_id);
  runtime.run_event_loop(Default::default()).await.unwrap();
  receiver.await.unwrap();

  // Verify it executed once
  runtime
    .execute_script("check1", "if (globalThis.executionCount !== 1) throw new Error('Expected 1 execution')")
    .unwrap();

  // Try to evaluate the same module again - this should not panic
  let receiver2 = runtime.mod_evaluate(mod_id);
  runtime.run_event_loop(Default::default()).await.unwrap();
  receiver2.await.unwrap();

  // Verify it still only executed once (module was not re-executed)
  runtime
    .execute_script("check2", "if (globalThis.executionCount !== 1) throw new Error('Expected still 1 execution')")
    .unwrap();
}

#[tokio::test]
async fn evaluate_already_evaluated_module_sync() {
  // This test verifies that calling mod_evaluate_sync on an already-evaluated module
  // doesn't panic, but instead returns Ok(()) immediately.

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::with(
    Url::parse("file:///main.js").unwrap(),
    ascii_str!(
      "globalThis.syncExecutionCount = (globalThis.syncExecutionCount || 0) + 1;"
    ),
  )));

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    ..Default::default()
  });

  let spec = resolve_url("file:///main.js").unwrap();

  // Load the module
  let mod_id = runtime.load_main_es_module(&spec).await.unwrap();

  // Evaluate synchronously using scope
  {
    let module_map = runtime.module_map();
    deno_core::scope!(scope, runtime);
    module_map.mod_evaluate_sync(scope, mod_id).unwrap();
  }

  // Verify it executed once
  runtime
    .execute_script("check1", "if (globalThis.syncExecutionCount !== 1) throw new Error('Expected 1 execution')")
    .unwrap();

  // Try to evaluate the same module again synchronously - should not panic
  {
    let module_map = runtime.module_map();
    deno_core::scope!(scope, runtime);
    module_map.mod_evaluate_sync(scope, mod_id).unwrap();
  }

  // Verify it still only executed once (module was not re-executed)
  runtime
    .execute_script("check2", "if (globalThis.syncExecutionCount !== 1) throw new Error('Expected still 1 execution')")
    .unwrap();
}

#[tokio::test]
async fn test_native_source_map_with_trailing_content() {
  // This test verifies that source maps work even with trailing newlines and comments
  // after the sourceMappingURL directive (https://github.com/denoland/deno/issues/21988)

  // Source map for TypeScript code that throws an error on line 2:
  // function greet(name: string) {
  //   throw new Error("Test error");
  // }
  //
  // greet("World");
  let source_map_base64 = "eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbImZpbGU6Ly8vdGVzdC50cyJdLCJzb3VyY2VzQ29udGVudCI6WyJmdW5jdGlvbiBncmVldChuYW1lOiBzdHJpbmcpIHtcbiAgdGhyb3cgbmV3IEVycm9yKFwiVGVzdCBlcnJvclwiKTtcbn1cblxuZ3JlZXQoXCJXb3JsZFwiKTtcbiJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiQUFBQTtBQUNBO0FBQ0E7QUFDQTtBQUNBIn0=";

  let code = format!(
    r#"export function greet(name) {{
  throw new Error("Test error");
}}

greet("World");

//# sourceMappingURL=data:application/json;base64,{}
// This is a comment after the sourceMappingURL

// Multiple blank lines follow


"#,
    source_map_base64
  );

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::with(
    Url::parse("file:///test.js").unwrap(),
    code,
  )));

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let spec = resolve_url("file:///test.js").unwrap();
  let mod_id = runtime.load_main_es_module(&spec).await.unwrap();
  let _receiver = runtime.mod_evaluate(mod_id);

  // Run event loop - it will propagate the module error
  let event_loop_result = runtime.run_event_loop(Default::default()).await;

  // The event loop returns the error from the uncaught promise rejection
  assert!(event_loop_result.is_err());
  let error = event_loop_result.unwrap_err();
  let error_string = error.to_string();

  // Verify that the error references the original TypeScript file and line number
  // The error should be thrown from line 2 of test.ts (not test.js)
  assert!(
    error_string.contains("test.ts"),
    "Error should reference source file test.ts, got: {}",
    error_string
  );
  assert!(
    error_string.contains("Test error"),
    "Error should contain the error message, got: {}",
    error_string
  );
}

// Module loader that supports loading external source maps
struct ExternalSourceMapLoader {
  module_code: String,
  source_map_content: String,
}

impl ModuleLoader for ExternalSourceMapLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    _module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let code = ModuleCodeString::from(self.module_code.clone());
    ModuleLoadResponse::Sync(Ok(ModuleSource::new(
      ModuleType::JavaScript,
      ModuleSourceCode::String(code),
      _module_specifier,
      None,
    )))
  }

  fn load_external_source_map(
    &self,
    source_map_url: &str,
  ) -> Option<Cow<'_, [u8]>> {
    // Check if this is the source map we're providing
    if source_map_url.ends_with("test.js.map")
      || source_map_url.contains("test.js.map")
    {
      Some(Cow::Borrowed(self.source_map_content.as_bytes()))
    } else {
      None
    }
  }
}

#[tokio::test]
async fn test_native_external_source_map_with_relative_path() {
  // Create a source map file for TypeScript code that throws an error
  // Original TypeScript:
  // export function throwError() {
  //   throw new Error("External source map test");
  // }
  // throwError();
  let source_map_content = r#"{
  "version": 3,
  "sources": ["file:///test.ts"],
  "sourcesContent": ["export function throwError() {\n  throw new Error(\"External source map test\");\n}\nthrowError();\n"],
  "names": [],
  "mappings": "AAAA;AACA;AACA;AACA"
}"#;

  // Create a JavaScript file that references the external source map with a relative path
  let js_code = r#"export function throwError() {
  throw new Error("External source map test");
}
throwError();

//# sourceMappingURL=./test.js.map"#;

  let file_url = Url::parse("file:///test.js").unwrap();

  let loader = Rc::new(TestingModuleLoader::new(ExternalSourceMapLoader {
    module_code: js_code.to_string(),
    source_map_content: source_map_content.to_string(),
  }));

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let mod_id = runtime.load_main_es_module(&file_url).await.unwrap();
  let _receiver = runtime.mod_evaluate(mod_id);

  // Run event loop - it will propagate the module error
  let event_loop_result = runtime.run_event_loop(Default::default()).await;

  // The event loop returns the error from the uncaught promise rejection
  assert!(event_loop_result.is_err());

  let error = event_loop_result.unwrap_err();
  let error_string = error.to_string();

  // Verify that the error references the original TypeScript file (not the .js file)
  // External source maps are loaded lazily when an error occurs
  assert!(
    error_string.contains("test.ts"),
    "Error should reference source file test.ts, got: {}",
    error_string
  );
  assert!(
    error_string.contains("External source map test"),
    "Error should contain the error message, got: {}",
    error_string
  );
}

#[tokio::test]
async fn test_native_source_map_with_absolute_path_outside_cwd() {
  // Test that source maps with absolute paths outside the cwd display as absolute
  // (not relative with excessive ../ sequences)

  // Create source map with absolute path outside typical cwd
  let source_map_base64 = "eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbImZpbGU6Ly8vYWJzb2x1dGUvcGF0aC9vdXRzaWRlL29yaWdpbmFsLnRzIl0sInNvdXJjZXNDb250ZW50IjpbIi8vIE9yaWdpbmFsIGZpbGUgb3V0c2lkZSBjd2RcbmZ1bmN0aW9uIHRocm93RXJyb3IoKSB7XG4gIHRocm93IG5ldyBFcnJvcihcIkVycm9yIGZyb20gZmlsZSBvdXRzaWRlIGN3ZFwiKTtcbn1cblxudGhyb3dFcnJvcigpO1xuIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQSJ9";

  let code = format!(
    r#"function throwError() {{
  throw new Error("Error from file outside cwd");
}}

throwError();

//# sourceMappingURL=data:application/json;base64,{}"#,
    source_map_base64
  );

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::with(
    Url::parse("file:///test.js").unwrap(),
    code,
  )));

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let mod_id = runtime
    .load_main_es_module(&Url::parse("file:///test.js").unwrap())
    .await
    .unwrap();

  // The module should load successfully even though the source map points
  // to a file outside the cwd. Error paths will show absolute paths.
  let _receiver = runtime.mod_evaluate(mod_id);
  let result = runtime.run_event_loop(Default::default()).await;

  // Expect an error since the code throws
  assert!(result.is_err());

  let err = result.unwrap_err();
  let err_str = err.to_string();

  // Verify the error contains the absolute path from the source map
  // (not relative with excessive ../)
  assert!(
    err_str.contains("/absolute/path/outside/original.ts"),
    "Error should contain absolute path: {}",
    err_str
  );
}

#[tokio::test]
async fn test_native_source_map_synthetic_bundle_path() {
  // Test that bundled code with synthetic paths (like file:///a.ts) displays correctly
  // This is common with bundlers like esbuild

  let source_map_base64 = "eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbImZpbGU6Ly8vYS50cyJdLCJzb3VyY2VzQ29udGVudCI6WyIvLyBCdW5kbGVkIGZpbGVcbmZ1bmN0aW9uIHRocm93RXJyb3IoKSB7XG4gIHRocm93IG5ldyBFcnJvcihcIkVycm9yIGZyb20gYnVuZGxlZCBmaWxlXCIpO1xufVxuXG50aHJvd0Vycm9yKCk7XG4iXSwibmFtZXMiOltdLCJtYXBwaW5ncyI6IkFBQUE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBIn0=";

  let code = format!(
    r#"function throwError() {{
  throw new Error("Error from bundled file");
}}

throwError();

//# sourceMappingURL=data:application/json;base64,{}"#,
    source_map_base64
  );

  let loader = Rc::new(TestingModuleLoader::new(StaticModuleLoader::with(
    Url::parse("file:///bundle.js").unwrap(),
    code,
  )));

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let mod_id = runtime
    .load_main_es_module(&Url::parse("file:///bundle.js").unwrap())
    .await
    .unwrap();

  let _receiver = runtime.mod_evaluate(mod_id);
  let result = runtime.run_event_loop(Default::default()).await;

  // Expect an error
  assert!(result.is_err());

  let err = result.unwrap_err();
  let err_str = err.to_string();

  // Verify the error shows the synthetic path from the bundle
  // Should show as absolute: /a.ts (not with excessive ../)
  assert!(
    err_str.contains("file:///a.ts"),
    "Error should contain synthetic bundle path: {}",
    err_str
  );

  // Should NOT contain excessive parent directory traversals
  assert!(
    !err_str.contains("../../../"),
    "Error should not contain excessive ../ sequences: {}",
    err_str
  );
}
