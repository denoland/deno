// Query read permissions - both the named set value and the CLI flag value
// should be granted when stacking `-P=foo -R=./data/bar.txt`
console.log(
  "read foo.txt:",
  Deno.permissions.querySync({ name: "read", path: "./data/foo.txt" }).state,
);
console.log(
  "read bar.txt:",
  Deno.permissions.querySync({ name: "read", path: "./data/bar.txt" }).state,
);
// A path not in either set should still be prompted
console.log(
  "read other:",
  Deno.permissions.querySync({ name: "read", path: "./other.txt" }).state,
);

// Query net permissions - both the named set value and the CLI flag value
// should be granted when stacking `-P=foo --allow-net=www.google.com`
console.log(
  "net example.org:",
  Deno.permissions.querySync({ name: "net", host: "example.org" }).state,
);
console.log(
  "net www.google.com:",
  Deno.permissions.querySync({ name: "net", host: "www.google.com" }).state,
);
// A host not in either set should still be prompted
console.log(
  "net other.com:",
  Deno.permissions.querySync({ name: "net", host: "other.com" }).state,
);
