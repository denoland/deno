console.log(Deno.permissions.querySync({ name: "read", path: "./data" }).state);
console.log(
  Deno.permissions.querySync({ name: "read", path: "./cli_data" }).state,
);
console.log(
  Deno.permissions.querySync({ name: "net", host: "example.com" }).state,
);
console.log(
  Deno.permissions.querySync({ name: "net", host: "localhost:4545" }).state,
);
