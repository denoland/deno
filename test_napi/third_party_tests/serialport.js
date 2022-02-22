const serialport = Deno.core.napiOpen(
  "node_modules/@serialport/bindings-cpp/prebuilds/darwin-x64+arm64/node.napi.node",
);
console.log(serialport.list(console.log));
