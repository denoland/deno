use crate::create_main_worker;
use crate::located_script_name;
use crate::module_graph::TypeLib;
use crate::program_state::ProgramState;
use crate::tools::test::TestEvent;
use crate::tools::test::TestProgram;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::ops::worker_host::create_worker_permissions;
use deno_runtime::ops::worker_host::PermissionsArg;
use deno_runtime::permissions::Permissions;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
struct PermissionsHolder(Uuid, Permissions);

pub fn init(rt: &mut JsRuntime) {
  super::reg_sync(rt, "op_pledge_test_permissions", op_pledge_test_permissions);
  super::reg_sync(
    rt,
    "op_restore_test_permissions",
    op_restore_test_permissions,
  );
  super::reg_sync(rt, "op_get_test_origin", op_get_test_origin);
  super::reg_sync(rt, "op_dispatch_test_event", op_dispatch_test_event);
  super::reg_async(rt, "op_run_test_program", op_run_test_program);
}

pub fn op_pledge_test_permissions(
  state: &mut OpState,
  args: PermissionsArg,
  _: (),
) -> Result<Uuid, AnyError> {
  deno_runtime::ops::check_unstable(state, "Deno.test.permissions");

  let token = Uuid::new_v4();
  let parent_permissions = state.borrow::<Permissions>().clone();
  let worker_permissions =
    create_worker_permissions(parent_permissions.clone(), args)?;

  state.put::<PermissionsHolder>(PermissionsHolder(token, parent_permissions));

  // NOTE: This call overrides current permission set for the worker
  state.put::<Permissions>(worker_permissions);

  Ok(token)
}

pub fn op_restore_test_permissions(
  state: &mut OpState,
  token: Uuid,
  _: (),
) -> Result<(), AnyError> {
  deno_runtime::ops::check_unstable(state, "Deno.test.permissions");

  if let Some(permissions_holder) = state.try_take::<PermissionsHolder>() {
    if token != permissions_holder.0 {
      panic!("restore test permissions token does not match the stored token");
    }

    let permissions = permissions_holder.1;
    state.put::<Permissions>(permissions);
    Ok(())
  } else {
    Err(generic_error("no permissions to restore"))
  }
}

fn op_get_test_origin(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<String, AnyError> {
  Ok(state.borrow::<ModuleSpecifier>().to_string())
}

fn op_dispatch_test_event(
  state: &mut OpState,
  event: TestEvent,
  _: (),
) -> Result<(), AnyError> {
  let sender = state.borrow::<Sender<TestEvent>>().clone();
  sender.send(event).ok();

  Ok(())
}

async fn op_run_test_program(
  state: Rc<RefCell<OpState>>,
  program: TestProgram,
  _: (),
) -> Result<(), AnyError> {
  // TODO(caspervonb): this is printing diagnostics during the type check, we should supress that.
  let program_state = state.borrow().borrow::<Arc<ProgramState>>().clone();
  let specifier = {
    let file = program.to_file();
    let specifier = file.specifier.clone();
    program_state.file_fetcher.insert_cached(file);

    specifier
  };

  let permissions = state.borrow().borrow::<Permissions>().clone();
  if program.no_run {
    let lib = TypeLib::UnstableDenoWindow;
    program_state
      .prepare_module_load(
        specifier,
        lib,
        Permissions::allow_all(),
        Permissions::allow_all(),
        false,
        program_state.maybe_import_map.clone(),
      )
      .await?;

    return Ok(());
  }

  let mut worker = create_main_worker(
    &program_state,
    specifier.clone(),
    permissions.clone(),
    None,
  );

  worker.execute_script(
    &located_script_name!(),
    "window.dispatchEvent(new Event('load'))",
  )?;

  worker.execute_module(&specifier).await?;

  worker.execute_script(
    &located_script_name!(),
    "window.dispatchEvent(new Event('unload'))",
  )?;

  Ok(())
}
