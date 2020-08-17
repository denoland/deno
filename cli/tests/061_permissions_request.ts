const status1 = await Deno.permissions.request({ name: "read", path: "foo" });
const status2 = await Deno.permissions.query({ name: "read", path: "bar" });
const status3 = await Deno.permissions.request({ name: "read", path: "bar" });
console.log(status1);
console.log(status2);
console.log(status3);
