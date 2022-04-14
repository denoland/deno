import { readableStreamFromReader } from "https://deno.land/std@0.134.0/streams/mod.ts";

const server = Deno.listen({ port: 8080 });
console.log("File server running on http://localhost:8080/");

for await (const conn of server) {
  handleHttp(conn);
}

async function handleHttp(conn) {
  const httpConn = Deno.serveHttp(conn);
  for await (const requestEvent of httpConn) {
    // Use the request pathname as filepath
    const url = new URL(requestEvent.request.url);
    const filepath = decodeURIComponent(url.pathname);

    requestEvent.sendFile("." + filepath);
    //
    // const file = await Deno.open("." + filepath, { read: true });
    // const readableStream = readableStreamFromReader(file);
    // const response = new Response(readableStream);
    // await requestEvent.respondWith(response);
  }
}
