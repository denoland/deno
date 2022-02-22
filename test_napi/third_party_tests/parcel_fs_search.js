const lib = Deno.core.napiOpen(
  "./node_modules/@parcel/fs-search/fs-search.darwin-arm64.node",
);

const file = lib.findFirstFile(
  [
    "./test/example_non_existent.js",
    "./test/example.js",
    "./test/example_non_existent2.js",
  ],
);

console.log(file); // ./test/example.js
