const xattr = Deno.core.dlopen(
  "node_modules/fs-xattr/build/Release/xattr.node",
);
xattr.set("exports.def", "foo", Deno.core.encode("bar")).then(console.log);

console.log(1)
//await xattr.get("exports.def", "foo");
