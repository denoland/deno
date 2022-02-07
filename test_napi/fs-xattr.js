const xattr = Deno.core.dlopen(
  "node_modules/fs-xattr/build/Release/xattr.node",
);
console.log(1)
await xattr.set("exports.def", "foo", Deno.core.encode("bar"));
await xattr.get("exports.def", "foo");
