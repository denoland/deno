const status1 =
  (await Deno.permissions.request({ name: "read", path: "foo" })).state;
const status2 =
  (await Deno.permissions.query({ name: "read", path: "bar" })).state;
const status3 = Deno.permissions.querySync({ name: "read", path: "bar" }).state;
const status4 =
  (await Deno.permissions.request({ name: "read", path: "bar" })).state;
console.log(status1);
console.log(status2);
console.log(status3);
console.log(status4);
