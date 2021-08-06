Deno.test({
  name: "ignore",
  permissions: {
    read: true,
    write: true,
    net: true,
    env: true,
    run: true,
    plugin: true,
    hrtime: true,
  },
  ignore: true,
  fn() {
    throw new Error("unreachable");
  },
});
