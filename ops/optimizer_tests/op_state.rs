fn op_set_exit_code(state: &mut OpState, code: i32) {
  state.borrow_mut::<ExitCode>().set(code);
}
