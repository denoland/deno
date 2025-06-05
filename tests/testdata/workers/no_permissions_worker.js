self.onmessage = async () => {
  const net = await Deno.permissions.query({ name: "net" });
  const ffi = await Deno.permissions.query({ name: "ffi" });
  const read = await Deno.permissions.query({ name: "read" });
  const run = await Deno.permissions.query({ name: "run" });
  const write = await Deno.permissions.query({ name: "write" });
  const imports = await Deno.permissions.query({ name: "import" });
  self.postMessage(
    net.state === "prompt" &&
      ffi.state === "prompt" &&
      read.state === "prompt" &&
      run.state === "prompt" &&
      write.state === "prompt" &&
      imports.state === "prompt",
  );
  self.close();
};
