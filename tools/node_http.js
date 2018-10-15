const http = require("http");
const port = process.argv[2] || "4544";
console.log("port", port);
http
  .Server((req, res) => {
    res.end("Hello World\n");
  })
  .listen(port);
