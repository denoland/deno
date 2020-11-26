const addr = Deno.args[0] || "0.0.0.0:4544";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
console.log("listening on", addr);
listener.accept().then(
  async (conn): Promise<void> => {
    console.log("recieved bytes:", await Deno.copy(conn, conn));
    conn.close();
    listener.close();
  },
);
