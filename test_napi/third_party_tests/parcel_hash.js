const lib = Deno.core.napiOpen(
  "./node_modules/@parcel/hash/parcel-hash.darwin-arm64.node",
);

console.log(lib.hashString("Hello, Deno!")); // 210a1f862b67f327
console.log(lib.hashBuffer(Deno.core.encode("Hello, Deno!"))); // 210a1f862b67f327

const hasher = new lib.Hash();
hasher.writeString("Hello, Deno!");
console.log(hasher.finish()); // 210a1f862b67f327
