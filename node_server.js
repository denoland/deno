const http = require("http");
const fs = require("fs");

http.createServer(function (req, res) {
    const filename = __dirname + req.url;

    const readStream = fs.createReadStream(filename);

    readStream.on("open", function() {
        readStream.pipe(res);
    });

    readStream.on("error", function(err) {
        res.end(err);
    })
}).listen(8081);
