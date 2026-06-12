// Deno.connect requires net-connect permission.
// We don't expect the connection itself to succeed (no listener) — only
// the permission check matters. Distinguish PermissionDenied from other
// errors.
let connected = false;
try {
  const conn = await Deno.connect({ hostname: "127.0.0.1", port: 1 });
  conn.close();
  connected = true;
} catch (e) {
  if (e instanceof Deno.errors.NotCapable) {
    throw e;
  }
  // any other error means the permission check passed
}
console.log(connected ? "connected" : "permission-ok");
