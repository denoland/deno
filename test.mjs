import http from "node:http";
import url from "node:url";

const server = http.createServer(function (request, response) {
  // Run the check function
  response.writeHead(200, {});
  response.end("ok");
  server.close();
});

globalThis.onunhandledrejection = function (err) {
  console.error(err);
  server.close();
};

server.listen(0, function () {
  const testURL = url.parse(
    `http://asdf:qwer@localhost:${this.address().port}`,
  );
  // The test here is if you set a specific authorization header in the
  // request we should not override that with basic auth
  testURL.headers = {
    Authorization: "NoAuthForYOU",
  };

  // make the request
  http.request(testURL, function (response) {
    console.log(response.statusCode);
  }).end();
});
