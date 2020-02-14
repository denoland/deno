const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const socket = Deno.listen({ hostname, port: Number(port), transport: "udp" });
const response = new TextEncoder().encode("Hello World");

console.log("Listening on", addr);
for await (const message of socket) {
  const [buffer, remote] = message;
  await socket.send(response, remote);
}
