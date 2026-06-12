// Tests that large response bodies are fully delivered through the
// DENO_SERVE_ADDRESS override listener, with and without a known
// content-length. Deno.Conn.write() may perform partial writes when the
// transport send buffer fills up (vsock buffers are 64 KiB), which the
// override socket must handle.
const OVERRIDE_PORT = 12473;
const SIZE = 32 * 1024 * 1024;

const child = new Deno.Command(Deno.execPath(), {
  args: ["run", "-A", "server.mjs"],
  env: {
    DENO_SERVE_ADDRESS: `duplicate,tcp:127.0.0.1:${OVERRIDE_PORT}`,
  },
  cwd: new URL(".", import.meta.url).pathname,
  stdout: "piped",
  stderr: "inherit",
}).spawn();

// Wait for the server to report readiness on stdout.
const reader = child.stdout.getReader();
let buf = "";
while (!buf.includes("listening")) {
  const { value, done } = await reader.read();
  if (done) break;
  buf += new TextDecoder().decode(value);
}

try {
  for (const path of ["/content-length", "/chunked"]) {
    const res = await fetch(`http://127.0.0.1:${OVERRIDE_PORT}${path}`, {
      headers: { "accept-encoding": "identity" },
    });
    const body = new Uint8Array(await res.arrayBuffer());
    let valid = true;
    for (let i = 0; i < body.length; i++) {
      if (body[i] !== 97) {
        valid = false;
        break;
      }
    }
    console.log(
      `${path}: status=${res.status} size_ok=${
        body.length === SIZE
      } content_ok=${valid}`,
    );
  }
} finally {
  child.kill();
  await child.status;
}
