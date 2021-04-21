# Simple HTTP web server

## Concepts

- Use Deno's integrated HTTP server to run your own web server.

## Overview

With just a few lines of code you can run your own HTTP web server with control
over the response status, request headers and more.

> ℹ️ The _native_ HTTP server is currently unstable, meaning the API is not
> finalized and may change in breaking ways in future version of Deno. To have
> the APIs discussed here available, you must run Deno with the `--unstable`
> flag.

## Sample web server

In this example, the user-agent of the client is returned to the client:

**webserver.ts**:

```ts
// Start listening on port 8080 of localhost.
const server = Deno.listen({ port: 8080 });
console.log(`HTTP webserver running.  Access it at:  http://localhost:8080/`);

// Connections to the server will be yielded up as an async iterable.
for await (const conn of server) {
  // In order to not be blocking, we need to handle each connection individually
  // in its own async function.
  (async () => {
    // This "upgrades" a network connection into an HTTP connection.
    const httpConn = Deno.serveHttp(conn);
    // Each request sent over the HTTP connection will be yielded as an async
    // iterator from the HTTP connection.
    for await (const requestEvent of httpConn) {
      // The native HTTP server uses the web standard `Request` and `Response`
      // objects.
      const body = `Your user-agent is:\n\n${requestEvent.request.headers.get(
        "user-agent",
      ) ?? "Unknown"}`;
      // The requestEvent's `.respondWith()` method is how we send the response
      // back to the client.
      requestEvent.respondWith(
        new Response(body, {
          status: 200,
        }),
      );
    }
  })();
}
```

Then run this with:

```shell
deno run --allow-net --unstable webserver.ts
```

Then navigate to `http://localhost:8080/` in a browser.

### Using the `std/http` library

If you do not want to use the unstable APIs, you can still use the standard
library's HTTP server:

**webserver.ts**:

```ts
import { serve } from "https://deno.land/std@$STD_VERSION/http/server.ts";

const server = serve({ port: 8080 });
console.log(`HTTP webserver running.  Access it at:  http://localhost:8080/`);

for await (const request of server) {
  let bodyContent = "Your user-agent is:\n\n";
  bodyContent += request.headers.get("user-agent") || "Unknown";

  request.respond({ status: 200, body: bodyContent });
}
```

Then run this with:

```shell
deno run --allow-net webserver.ts
```
