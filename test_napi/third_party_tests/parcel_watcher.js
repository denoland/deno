const watcher = Deno.core.napiOpen(
  "node_modules/@parcel/watcher/prebuilds/darwin-arm64/node.napi.glibc.node",
);

await watcher.subscribe("node_modules/", (err, event) => {
  console.log(event);
}, {});
