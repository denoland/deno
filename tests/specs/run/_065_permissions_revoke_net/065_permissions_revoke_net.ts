const status1 = await Deno.permissions.query({ name: "net" });
console.log(status1);
const status2 = await Deno.permissions.revoke({ name: "net" });
console.log(status2);
const status3 = await Deno.permissions.query({ name: "net" });
console.log(status3);
