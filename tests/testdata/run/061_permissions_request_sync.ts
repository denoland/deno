const status1 =
  Deno.permissions.requestSync({ name: "read", path: "foo" }).state;
const status2 = Deno.permissions.querySync({ name: "read", path: "bar" }).state;
const status3 =
  Deno.permissions.requestSync({ name: "read", path: "bar" }).state;
console.log(status1);
console.log(status2);
console.log(status3);
