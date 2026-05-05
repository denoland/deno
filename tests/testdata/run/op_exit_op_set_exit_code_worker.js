self.onmessage = () => {
  Deno.exitCode = 42;
  Deno.exit();
};
