console.log(await Deno.permissions.revoke({ name: "read", path: "foo" }));
console.log(await Deno.permissions.query({ name: "read", path: "bar" }));
console.log(await Deno.permissions.revoke({ name: "read", path: "bar" }));
