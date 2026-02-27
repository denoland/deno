// Copyright 2018-2025 the Deno authors. MIT license.

mod checkin;

pub use checkin::runner::create_runtime_from_snapshot;
pub use checkin::runner::create_runtime_from_snapshot_with_options;
pub use checkin::runner::snapshot::create_snapshot;

macro_rules! unit_test {
  ($($id:ident,)*) => {
    #[cfg(test)]
    mod unit {
      $(
        #[test]
        fn $id() {
          $crate::checkin::runner::testing::run_unit_test(stringify!($id));
        }
      )*
    }
  };
}

macro_rules! integration_test {
  ($($id:ident,)*) => {
    #[cfg(test)]
    mod integration {
      $(
        #[test]
        fn $id() {
          $crate::checkin::runner::testing::run_integration_test(stringify!($id));
        }
      )*
    }
  };
}

// Test individual bits of functionality. These files are loaded from the unit/ dir.
unit_test!(
  encode_decode_test,
  error_test,
  microtask_test,
  ops_async_test,
  ops_buffer_test,
  ops_error_test,
  resource_test,
  serialize_deserialize_test,
  stats_test,
  task_test,
  tc39_test,
  timer_test,
  type_test,
  callsite_test,
);

// Test the load and run of an entire file within the `checkin` infrastructure.
// These files are loaded from the integration/ dir.
integration_test!(
  builtin_console_test,
  dyn_import_circular,
  dyn_import_op,
  dyn_import_no_hang,
  dyn_import_pending_tla,
  error_async_stack,
  error_callsite,
  error_non_existent_eval_source,
  error_rejection_catch,
  error_rejection_order,
  error_eval_stack,
  error_ext_stack,
  error_prepare_stack_trace,
  error_prepare_stack_trace_crash,
  error_source_maps_with_prepare_stack_trace,
  error_with_stack,
  error_without_stack,
  error_get_file_name,
  error_get_file_name_to_string,
  error_get_script_name_or_source_url,
  import_sync,
  import_sync_existing,
  import_sync_throw,
  main_module_handler,
  module_types,
  pending_unref_op_tla,
  smoke_test,
  source_phase_imports,
  source_phase_imports_dynamic,
  timer_ref,
  timer_ref_and_cancel,
  timer_many,
  ts_types,
  user_breaks_promise_constructor,
  user_breaks_promise_species,
  wasm_imports,
  wasm_stack_trace,
  worker_spawn,
  worker_terminate,
  worker_terminate_op,
);
