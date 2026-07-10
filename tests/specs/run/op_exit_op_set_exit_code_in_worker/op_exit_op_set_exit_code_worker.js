self.onmessage = () => {
  self.close = () => {
    self.postMessage("overridden self.close was called");
  };
  Deno[Deno.internal].core.ops.op_set_exit_code(42);
  Deno.exit();
};
