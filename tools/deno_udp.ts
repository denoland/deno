// Used for benchmarking Deno's networking. See tools/http_benchmark.py
// TODO Replace this with a real HTTP server once
// https://github.com/denoland/deno/issues/726 is completed.
// Note: this is a keep-alive server.
const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const socket = Deno.listen({ hostname, port: Number(port), transport: "udp" });
const response = new TextEncoder().encode("Hello World");

console.log("Listening on", addr);
for await (const message of socket) {
  const [buffer, remote] = message;
  await socket.send(buffer, remote);
}
