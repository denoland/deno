import { serve } from "../server.ts";
import { serveFile } from "../file_server.ts";

const server = serve({ port: 8000 });

console.log("Server running...");

for await (const req of server) {
  serveFile(req, "./testdata/hello.html").then((response) => {
    req.respond(response);
  });
}
