const watcher = Deno.core.napiOpen(
  "node_modules/@parcel/watcher/prebuilds/darwin-arm64/node.napi.glibc.node",
);
await watcher.subscribe("node_modules/", console.log, {}).then(console.log)
  .catch(console.log);
