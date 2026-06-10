import http from "node:http";
import { text } from "node:stream/consumers";

const server = http.createServer((req, res) => {
  res.end("foo");
});

server.listen(0, async () => {
  const port = server.address().port;
  for (const _ of Array(3)) {
    await new Promise((resolve) => {
      http.get(`http://localhost:${port}`, async (res) => {
        await text(res);
        resolve();
      });
    });
  }
  server.close();
});
