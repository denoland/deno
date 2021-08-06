self.onmessage = async () => {
  const hrtime = await Deno.permissions.query({ name: "hrtime" });
  const net = await Deno.permissions.query({ name: "net" });
  const ffi = await Deno.permissions.query({ name: "ffi" });
  const read = await Deno.permissions.query({ name: "read" });
  const run = await Deno.permissions.query({ name: "run" });
  const write = await Deno.permissions.query({ name: "write" });
  self.postMessage(
    hrtime.state === "denied" &&
      net.state === "denied" &&
      ffi.state === "denied" &&
      read.state === "denied" &&
      run.state === "denied" &&
      write.state === "denied",
  );
  self.close();
};
