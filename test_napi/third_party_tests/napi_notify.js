const notify = Deno.core.dlopen(
  "node_modules/@napi-rs/notify-darwin-arm64/notify.darwin-arm64.node",
);
const unwatch = notify.watch(".", (err, event) => {
  console.log(err, event);
});
