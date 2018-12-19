# net

Usage:

```typescript
import { serve } from "https://deno.land/x/net/http.ts";
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
alias file_server="deno https://deno.land/x/net/file_server.ts --allow-net"
```
