import { serveFile } from "../file_server.ts";

Deno.serve(
  { port: 8000, onListen: () => console.log("Server running...") },
  (req) => {
    return serveFile(req, "./testdata/hello.html");
  },
);
