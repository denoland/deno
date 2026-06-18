// Deno.listen on 127.0.0.1:0 — requires net-listen permission.
const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
console.log(
  `listening on ${listener.addr.transport}://${listener.addr.hostname}:${listener.addr.port}`,
);
listener.close();
