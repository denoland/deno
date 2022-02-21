const xattr = Deno.core.dlopen(
  "node_modules/fs-xattr/build/Release/xattr.node",
);

const p = xattr.set("exports.def", "foo", Deno.core.encode("bar")).catch(
  console.error,
);
console.log("Probably logged before");
await p;
console.log("Definitely logged after");
