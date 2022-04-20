const http = require("http");
const fs = require("fs");
const path = require("path");
const os = require("os");

const port = process.argv[2] || "4544";
console.log("port", port);

const tempFile = path.join(os.tmpdir(), "temp.txt");
fs.writeFileSync(tempFile, new Uint8Array(1024 * 1024 * 5).fill(0)); // 5MB

http.createServer(function (req, res) {
  const readStream = fs.createReadStream(tempFile);
  readStream.on("open", function () {
    readStream.pipe(res);
  });
  readStream.on("error", function (err) {
    res.end(err);
  });
}).listen(port);
