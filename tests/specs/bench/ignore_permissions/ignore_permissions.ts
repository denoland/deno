Deno.bench({
  name: "ignore",
  permissions: {
    read: true,
    write: true,
    net: true,
    env: true,
    run: true,
    ffi: true,
  },
  ignore: true,
  fn() {
    throw new Error("unreachable");
  },
});
