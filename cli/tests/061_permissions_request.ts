console.log(await Deno.permissions.request({ name: "read", path: "foo" }));
console.log(await Deno.permissions.query({ name: "read", path: "bar" }));
console.log(await Deno.permissions.request({ name: "read", path: "bar" }));
