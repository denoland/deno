const status1 = await Deno.permissions.revoke({ name: "read", path: "foo" });
console.log(status1);
const status2 = await Deno.permissions.query({ name: "read", path: "bar" });
console.log(status2);
const status3 = await Deno.permissions.revoke({ name: "read", path: "bar" });
console.log(status3);
