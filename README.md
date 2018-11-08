# Deno Networking Libraries

[![Build Status](https://travis-ci.com/denoland/net.svg?branch=master)](https://travis-ci.com/denoland/net)

Usage:

```typescript
import { serve } from "https://deno.land/x/net/http.ts";
const s = serve("0.0.0.0:8000");
for await (const req of s) {
  req.respond({ body: "Hello World\n" });
}
```
