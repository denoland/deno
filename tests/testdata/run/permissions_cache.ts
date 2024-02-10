const status = await Deno.permissions.query({ name: "read", path: "foo" });
console.log(status.state);
status.onchange = () => console.log(status.state);
await Deno.permissions.request({ name: "read", path: "foo" }); // y
await Deno.permissions.revoke({ name: "read", path: "foo" });
