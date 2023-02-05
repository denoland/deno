Deno.permissions.request({ name: "run", command: "FOO" });
Deno.permissions.request({ name: "run", command: "BAR" });

Deno.permissions.request({ name: "env", variable: "FOO" });
Deno.permissions.request({ name: "env", variable: "BAR" });
