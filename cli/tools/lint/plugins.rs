// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::PollEventLoopOptions;
use deno_core::RuntimeOptions;
use indexmap::IndexMap;
use std::rc::Rc;

pub async fn load_plugins(ast_string: String) -> Result<(), AnyError> {
  let plugin_file_path = "./plugin.js";

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![deno_lint_ext::init_ops()],
    module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
    ..Default::default()
  });

  let obj = runtime.lazy_load_es_module_with_code(
    "ext:cli/lint.js",
    deno_core::ascii_str_include!(concat!("lint.js")),
  )?;

  let run_plugin_rule_fn = {
    let scope = &mut runtime.handle_scope();
    let fn_name = v8::String::new(scope, "runPluginRule").unwrap();
    let obj_local: v8::Local<v8::Object> =
      v8::Local::new(scope, obj).try_into().unwrap();
    let run_fn_val = obj_local.get(scope, fn_name.into()).unwrap();
    let run_fn: v8::Local<v8::Function> = run_fn_val.try_into().unwrap();
    v8::Global::new(scope, run_fn)
  };

  let mut specifier =
    Url::from_directory_path(std::env::current_dir().unwrap())
      .unwrap()
      .join(&plugin_file_path)
      .unwrap();
  let mod_id = runtime.load_side_es_module(&specifier).await?;
  let mod_future = runtime.mod_evaluate(mod_id).boxed_local();
  runtime
    .run_event_loop(PollEventLoopOptions::default())
    .await?;
  let module = mod_future.await?;

  let rules_to_run = {
    let op_state = runtime.op_state();
    let state = op_state.borrow();
    let container = state.borrow::<LintPluginContainer>();

    eprintln!("Loaded plugins:");
    for (key, plugin) in container.plugins.iter() {
      eprintln!(" - {}", key);
      for rule in plugin.rules.keys() {
        eprintln!("   - {}", rule);
      }
    }

    let mut to_run = vec![];

    for (plugin_name, plugin) in container.plugins.iter() {
      for rule_name in plugin.rules.keys() {
        to_run.push((plugin_name.to_string(), rule_name.to_string()));
      }
    }

    to_run
  };

  for (plugin_name, rule_name) in rules_to_run {
    let (file_name, plugin_name_v8, rule_name_v8, ast_string_v8) = {
      let scope = &mut runtime.handle_scope();
      let file_name: v8::Local<v8::Value> =
        v8::String::new(scope, "foo.js").unwrap().into();
      let plugin_name_v8: v8::Local<v8::Value> =
        v8::String::new(scope, &plugin_name).unwrap().into();
      let rule_name_v8: v8::Local<v8::Value> =
        v8::String::new(scope, &rule_name).unwrap().into();
      let ast_string_v8: v8::Local<v8::Value> =
        v8::String::new(scope, &ast_string).unwrap().into();
      (
        v8::Global::new(scope, file_name),
        v8::Global::new(scope, plugin_name_v8),
        v8::Global::new(scope, rule_name_v8),
        v8::Global::new(scope, ast_string_v8),
      )
    };
    let call = runtime.call_with_args(
      &run_plugin_rule_fn,
      &[file_name, plugin_name_v8, rule_name_v8, ast_string_v8],
    );
    let result = runtime
      .with_event_loop_promise(call, PollEventLoopOptions::default())
      .await;
    match result {
      Ok(r) => {
        eprintln!("plugin finished")
      }
      Err(error) => {
        eprintln!("error running plugin {}", error);
      }
    }
  }

  Ok(())
}

struct LintPluginDesc {
  rules: IndexMap<String, v8::Global<v8::Function>>,
}

#[derive(Default)]
struct LintPluginContainer {
  plugins: IndexMap<String, LintPluginDesc>,
}

impl LintPluginContainer {
  fn register(
    &mut self,
    name: String,
    desc: LintPluginDesc,
  ) -> Result<(), AnyError> {
    if self.plugins.contains_key(&name) {
      return Err(custom_error(
        "AlreadyExists",
        format!("{} plugin already exists", name),
      ));
    }

    self.plugins.insert(name, desc);
    Ok(())
  }
}

deno_core::extension!(
  deno_lint_ext,
  ops = [
    op_lint_register_lint_plugin,
    op_lint_register_lint_plugin_rule,
    op_lint_get_rule
  ],
  state = |state| {
    state.put(LintPluginContainer::default());
  },
);

#[op2(fast)]
fn op_lint_register_lint_plugin(
  state: &mut OpState,
  #[string] name: String,
) -> Result<(), AnyError> {
  let plugin_desc = LintPluginDesc {
    rules: IndexMap::new(),
  };
  let container = state.borrow_mut::<LintPluginContainer>();
  container.register(name, plugin_desc)?;
  Ok(())
}

#[op2]
fn op_lint_register_lint_plugin_rule(
  state: &mut OpState,
  #[string] plugin_name: &str,
  #[string] name: String,
  #[global] create: v8::Global<v8::Function>,
) -> Result<(), AnyError> {
  let container = state.borrow_mut::<LintPluginContainer>();
  let mut plugin_desc = container.plugins.get_mut(plugin_name).unwrap();
  if plugin_desc.rules.contains_key(&name) {
    return Err(custom_error(
      "AlreadyExists",
      format!("{} plugin already exists", name),
    ));
  }
  plugin_desc.rules.insert(name, create);
  Ok(())
}

#[op2]
#[global]
fn op_lint_get_rule(
  state: &mut OpState,
  #[string] plugin_name: &str,
  #[string] rule_name: &str,
) -> Result<v8::Global<v8::Function>, AnyError> {
  let container = state.borrow::<LintPluginContainer>();
  let Some(plugin) = container.plugins.get(plugin_name) else {
    return Err(custom_error(
      "NotFound",
      format!("{} plugin is not registered", plugin_name),
    ));
  };
  let Some(rule) = plugin.rules.get(rule_name) else {
    return Err(custom_error(
      "NotFound",
      format!("Plugin {}, does not have {} rule", plugin_name, rule_name),
    ));
  };
  Ok(rule.clone())
}
