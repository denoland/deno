self.onmessage = async () => {
  const hrtime = await Deno.permissions.query({ name: "hrtime" });
  const net = await Deno.permissions.query({ name: "net" });
  const plugin = await Deno.permissions.query({ name: "plugin" });
  const read = await Deno.permissions.query({ name: "read" });
  const run = await Deno.permissions.query({ name: "run" });
  const write = await Deno.permissions.query({ name: "write" });
  self.postMessage(
    hrtime.state === "denied" &&
      net.state === "denied" &&
      plugin.state === "denied" &&
      read.state === "denied" &&
      run.state === "denied" &&
      write.state === "denied",
  );
  self.close();
};
