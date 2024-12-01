// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::tools::test::TestContainer;
use crate::tools::test::TestDescription;
use crate::tools::test::TestEvent;
use crate::tools::test::TestEventSender;
use crate::tools::test::TestFailure;
use crate::tools::test::TestLocation;
use crate::tools::test::TestStepDescription;
use crate::tools::test::TestStepResult;

use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::PollEventLoopOptions;
use deno_core::RuntimeOptions;
use deno_runtime::deno_permissions::ChildPermissionsArg;
use deno_runtime::deno_permissions::PermissionsContainer;
use indexmap::IndexMap;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use uuid::Uuid;

pub async fn load_plugins() -> Result<(), AnyError> {
  let plugin_file_path = "./plugin.js";

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![deno_lint_ext::init_ops()],
    module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
    ..Default::default()
  });

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

  let op_state = runtime.op_state();
  let state = op_state.borrow();
  let container = state.borrow::<LintPluginContainer>();

  eprintln!("Loaded plugins:");
  for key in container.plugins.keys() {
    eprintln!(" - {}", key);
  }
  Ok(())
}

struct LintPluginDesc {
  create: v8::Global<v8::Function>,
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
  ops = [op_register_lint_plugin],
  state = |state| {
    state.put(LintPluginContainer::default());
  },
);

#[op2]
fn op_register_lint_plugin(
  state: &mut OpState,
  #[string] name: String,
  #[global] create: v8::Global<v8::Function>,
) -> Result<(), AnyError> {
  let plugin_desc = LintPluginDesc { create };
  let container = state.borrow_mut::<LintPluginContainer>();
  container.register(name, plugin_desc)?;
  Ok(())
}
