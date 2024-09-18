Deno.permissions.requestSync({ name: "run", command: "FOO" });
Deno.permissions.requestSync({ name: "run", command: "BAR" });

Deno.permissions.requestSync({ name: "read", path: "FOO" });
Deno.permissions.requestSync({ name: "read", path: "BAR" });

Deno.permissions.requestSync({ name: "write", path: "FOO" });
Deno.permissions.requestSync({ name: "write", path: "BAR" });

Deno.permissions.requestSync({ name: "net", host: "FOO" });
Deno.permissions.requestSync({ name: "net", host: "BAR" });

Deno.permissions.requestSync({ name: "env", variable: "FOO" });
Deno.permissions.requestSync({ name: "env", variable: "BAR" });

Deno.permissions.requestSync({ name: "sys", kind: "loadavg" });
Deno.permissions.requestSync({ name: "sys", kind: "hostname" });

Deno.permissions.requestSync({ name: "ffi", path: "FOO" });
Deno.permissions.requestSync({ name: "ffi", path: "BAR" });
