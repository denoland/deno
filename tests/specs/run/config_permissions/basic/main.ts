console.log(Deno.permissions.querySync({ name: "read", path: "./data" }).state);
console.log(Deno.permissions.querySync({ name: "read", path: "." }).state);
console.log(
  Deno.permissions.querySync({ name: "write", path: "./data" }).state,
);
console.log(Deno.permissions.querySync({ name: "write", path: "." }).state);
