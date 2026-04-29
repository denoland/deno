// An unref'd TCP server should not keep the event loop alive.
import net from "node:net";

const server = net.createServer(() => {});
server.listen(0, () => {
  server.unref();
  console.log("server unref'd");
  // Process should exit naturally since the server is unref'd
});
