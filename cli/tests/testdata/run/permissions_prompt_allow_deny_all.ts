Deno.permissions.request({ name: "run", command: "FOO" });
Deno.permissions.request({ name: "run", command: "BAR" });

Deno.permissions.request({ name: "read", path: "FOO" });
Deno.permissions.request({ name: "read", path: "BAR" });

Deno.permissions.request({ name: "write", path: "FOO" });
Deno.permissions.request({ name: "write", path: "BAR" });

Deno.permissions.request({ name: "env", variable: "FOO" });
Deno.permissions.request({ name: "env", variable: "BAR" });
