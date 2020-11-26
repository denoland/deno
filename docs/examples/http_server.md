# Simple HTTP web server

## Concepts

- Use the std library [http module](https://deno.land/std@$STD_VERSION/http) to
  run your own web server.

## Overview

With just a few lines of code you can run your own http web server with control
over the response status, request headers and more.

## Sample web server

In this example, the user-agent of the client is returned to the client:

```typescript
/**
 * webserver.ts
 */
import { serve } from "https://deno.land/std@$STD_VERSION/http/server.ts";

const server = serve({ hostname: "0.0.0.0", port: 8080 });
console.log(`HTTP webserver running.  Access it at:  http://localhost:8080/`);

for await (const request of server) {
  let bodyContent = "Your user-agent is:\n\n";
  bodyContent += request.headers.get("user-agent") || "Unknown";

  request.respond({ status: 200, body: bodyContent });
}
```

Run this with:

```shell
deno run --allow-net webserver.ts
```
