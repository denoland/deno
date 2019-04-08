# http

A framework for creating HTTP/HTTPS server.

## Example

```typescript
import { serve } from "https://deno.land/std/http/server.ts";
const s = serve("0.0.0.0:8000");

async function main() {
  for await (const req of s) {
    req.respond({ body: new TextEncoder().encode("Hello World\n") });
  }
}

main();
```

### File Server

A small program for serving local files over HTTP.

Add the following to your `.bash_profile`

```
alias file_server="deno --allow-net https://deno.land/std/http/file_server.ts"
```
