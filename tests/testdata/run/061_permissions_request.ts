const status1 =
  (await Deno.permissions.request({ name: "read", path: "foo" })).state;
const status2 =
  (await Deno.permissions.query({ name: "read", path: "bar" })).state;
const status3 =
  (await Deno.permissions.request({ name: "read", path: "bar" })).state;
console.log(status1);
console.log(status2);
console.log(status3);
