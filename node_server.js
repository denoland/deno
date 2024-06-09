import http from "node:http";
import url from "node:url";

const server = http.createServer(function (request, response) {
  // Run the check function
  console.log(request.url);
  response.writeHead(200, {});
  response.end("ok");
  server.close();
});

server.listen(0, function () {
  // console.log("server listening", this.address().port);
  const testURL = url.parse(`http://localhost:${this.address().port}/asdf`);

  // // make the request
  http.request(testURL).end();
  // setTimeout(() => http.request(testURL).end(), 1000);
  // req.on("error", (e) => {
  //   console.log("error in req", req);
  // });

  // req.end();
});

// setTimeout(() => {}, 100_000);
