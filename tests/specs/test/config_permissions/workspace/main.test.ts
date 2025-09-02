console.log(
  "Root:",
  Deno.permissions.querySync({
    name: "read",
    path: import.meta.dirname + "/data",
  }).state,
);
console.log(
  "Member:",
  Deno.permissions.querySync({
    name: "read",
    path: import.meta.dirname + "/a/data",
  }).state,
);
