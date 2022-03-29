self.onmessage = async () => {
  const env = await Deno.permissions.query({ name: "env" });
  const ffi = await Deno.permissions.query({ name: "ffi" });
  const hrtime = await Deno.permissions.query({ name: "hrtime" });
  const net = await Deno.permissions.query({ name: "net" });
  const read = await Deno.permissions.query({ name: "read" });
  const run = await Deno.permissions.query({ name: "run" });
  const write = await Deno.permissions.query({ name: "write" });
  self.postMessage({
    env: env.state,
    ffi: ffi.state,
    hrtime: hrtime.state,
    net: net.state,
    read: read.state,
    run: run.state,
    write: write.state,
  });
  self.close();
};
