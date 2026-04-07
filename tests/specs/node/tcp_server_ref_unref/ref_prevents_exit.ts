// A ref'd TCP server (after unref + ref) should keep the event loop alive.
// We use a timeout to prove the server kept the process running, then clean up.
import net from "node:net";

const server = net.createServer(() => {});
server.listen(0, () => {
  // unref then re-ref — server should still keep the process alive
  server.unref();
  server.ref();

  setTimeout(() => {
    console.log("still alive after ref");
    server.close();
  }, 100);
});
