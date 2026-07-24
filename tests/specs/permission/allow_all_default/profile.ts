// Allow-all default permission profile (gate on, no flags): every capability
// is granted, exactly as if -A had been passed.
const names = ["read", "write", "net", "env", "run", "sys", "ffi"] as const;
for (const name of names) {
  const status = await Deno.permissions.query({ name });
  console.log(`${name}: ${status.state}`);
}
