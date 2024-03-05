import http2 from "node:http2";

const server = http2.createServer((req, res) => {
  console.log("handler called");
  console.log(req);
  res.setHeader("Content-Type", "text/html");
  res.setHeader("X-Foo", "bar");
  res.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
  res.write("Hello, World!");
  console.log(res);
  res.end();
});

server.listen(8000);
