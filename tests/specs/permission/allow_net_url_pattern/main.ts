// Tests URLPattern-style entries in --allow-net. The allow list grants
//   http://127.0.0.1:9999/api/*
// which constrains scheme, host, port and path. We classify each access by
// whether the permission layer rejects it (NotCapable) before any connection
// is attempted. Allowed accesses fail instead with a connection error against
// the closed 127.0.0.1:9999 port, which is immediate and offline-safe.

function isPermissionError(e: unknown): boolean {
  return e instanceof Deno.errors.NotCapable ||
    (e instanceof Error && e.name === "PermissionDenied");
}

async function classifyFetch(url: string): Promise<string> {
  try {
    await fetch(url);
    return "allowed";
  } catch (e) {
    return isPermissionError(e) ? "denied" : "allowed";
  }
}

async function classifyConnect(
  hostname: string,
  port: number,
): Promise<string> {
  try {
    const conn = await Deno.connect({ hostname, port });
    conn.close();
    return "allowed";
  } catch (e) {
    return isPermissionError(e) ? "denied" : "allowed";
  }
}

// Matching path on the granted scheme/host/port -> allowed.
console.log("api path:", await classifyFetch("http://127.0.0.1:9999/api/x"));
// Path outside the granted prefix -> denied.
console.log("other path:", await classifyFetch("http://127.0.0.1:9999/other"));
// Wrong scheme (https vs http) -> denied.
console.log(
  "wrong scheme:",
  await classifyFetch("https://127.0.0.1:9999/api/x"),
);
// Wrong port -> denied.
console.log("wrong port:", await classifyFetch("http://127.0.0.1:8888/api/x"));
// Raw socket access is never granted by a URL pattern entry -> denied.
console.log("raw connect:", await classifyConnect("127.0.0.1", 9999));
