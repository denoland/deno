# http

A framework for creating HTTP/HTTPS server.

## Example

```typescript
import { createServer } from "https://deno.land/x/http/server.ts";
import { encode } from "https://deno.land/x/strings/strings.ts";

async function main() {
  const server = createServer();
  server.handle("/", async (req, res) => {
    await res.respond({
      status: 200,
      body: encode("ok")
    });
  });
  server.handle(new RegExp("/foo/(?<id>.+)"), async (req, res) => {
    const { id } = req.match.groups;
    await res.respondJson({ id });
  });
  server.listen("127.0.0.1:8080");
}

main();
```

### File Server

A small program for serving local files over HTTP.

Add the following to your `.bash_profile`

```
alias file_server="deno https://deno.land/x/http/file_server.ts --allow-net"
```
