// Automatic TLS certificate provisioning via the mock ACME CA running in
// test_server (directory on port 4270, http-01 validation on port 4271).
const cacheDir = await Deno.makeTempDir();

const server = Deno.serve({
  port: 12370,
  hostname: "localhost",
  acme: {
    domains: ["localhost"],
    directoryUrl: "http://localhost:4270/acme/directory",
    contact: "test@example.com",
    cacheDir,
    challengePort: 4271,
  },
  onListen: () => console.log("serving"),
}, () => new Response("hello from acme tls"));

let lastErr: unknown = null;
for (let i = 0; i < 120; i++) {
  try {
    const res = await fetch("https://localhost:12370/");
    console.log("got:", res.status, JSON.stringify(await res.text()));
    await server.shutdown();
    await Deno.remove(cacheDir, { recursive: true });
    console.log("OK");
    Deno.exit(0);
  } catch (e) {
    lastErr = e;
    await new Promise((r) => setTimeout(r, 500));
  }
}
console.error("FAILED:", lastErr);
Deno.exit(1);
