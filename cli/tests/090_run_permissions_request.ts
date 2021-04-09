const status1 =
  (await Deno.permissions.request({ name: "run", command: "ls" })).state;
const status2 =
  (await Deno.permissions.query({ name: "run", command: "cat" })).state;
const status3 =
  (await Deno.permissions.request({ name: "run", command: "cat" })).state;
console.log(status1);
console.log(status2);
console.log(status3);
