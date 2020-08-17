console.log(await Deno.permissions.request({ name: "read" }));
console.log(await Deno.permissions.query({ name: "read", path: "foo" }));
console.log(await Deno.permissions.query({ name: "read", path: "bar" }));
