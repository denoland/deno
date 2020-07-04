const addr = Deno.args[1] || "0.0.0.0:4544";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
console.log("listening on", addr);
listener.accept().then(
  async (conn): Promise<void> => {
    await Deno.copy(conn, conn);
    conn.close();
    listener.close();
  }
);
