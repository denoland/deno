console.log(Deno.permissions.querySync({ name: "env" }));
console.log(Deno.permissions.querySync({ name: "read" }));
console.log(Deno.permissions.querySync({ name: "write" }));
console.log(Deno.permissions.querySync({ name: "ffi" }));
console.log(Deno.permissions.querySync({ name: "run" }));
console.log(Deno.permissions.querySync({ name: "sys" }));
console.log(Deno.permissions.querySync({ name: "net" }));
