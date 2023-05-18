const status1 = Deno.permissions.revokeSync({ name: "read" });
console.log(status1);
const status2 = Deno.permissions.querySync({ name: "read", path: "foo" });
console.log(status2);
const status3 = Deno.permissions.querySync({ name: "read", path: "bar" });
console.log(status3);
