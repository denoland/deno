import { createServer } from "node:http";
const SIZE = 32 * 1024 * 1024;
const big = Buffer.alloc(SIZE, "a");
const server = createServer((req, res) => {
  res.setHeader("content-type", "application/octet-stream");
  if (req.url === "/chunked") {
    // No content-length: chunked transfer encoding, written in pieces
    // large enough to hit partial transport writes.
    for (let i = 0; i < SIZE; i += 1024 * 1024) {
      res.write(big.subarray(i, i + 1024 * 1024));
    }
    res.end();
  } else {
    res.setHeader("content-length", SIZE);
    res.end(big);
  }
});
server.listen(8000, () => console.log("listening"));
