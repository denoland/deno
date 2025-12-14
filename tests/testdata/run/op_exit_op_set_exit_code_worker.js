self.onmessage = () => {
  Deno.exit(42);
  Deno.exit();
};
