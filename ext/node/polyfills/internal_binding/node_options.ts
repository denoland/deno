// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const {
  op_node_options_get_exec_argv_options,
  op_node_options_get_options,
  op_node_options_set_exec_argv,
} = __bootstrap.core.ops;

return {
  getExecArgvOptions: op_node_options_get_exec_argv_options,
  getOptions: op_node_options_get_options,
  setOptionSourceExecArgv: op_node_options_set_exec_argv,
};
})();
