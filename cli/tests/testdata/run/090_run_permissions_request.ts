const status1 =
  (await Deno.permissions.request({ name: "run", command: "ls" })).state;
if (status1 != "granted") {
  throw Error(`unexpected status1 ${status1}`);
}
const status2 =
  (await Deno.permissions.query({ name: "run", command: "cat" })).state;
if (status2 != "prompt") {
  throw Error(`unexpected status2 ${status2}`);
}
const status3 =
  (Deno.permissions.querySync({ name: "run", command: "cat" })).state;
if (status3 != "prompt") {
  throw Error(`unexpected status3 ${status3}`);
}
const status4 =
  (await Deno.permissions.request({ name: "run", command: "cat" })).state;
if (status4 != "denied") {
  throw Error(`unexpected status3 ${status4}`);
}
console.log(status1);
console.log(status2);
console.log(status3);
console.log(status4);
