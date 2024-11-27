self.onmessage = () => {
  Deno[Deno.internal].core.ops.op_set_exit_code(42);
  Deno.exit();
};
