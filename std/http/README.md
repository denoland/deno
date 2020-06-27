# http

```typescript
import { serve } from "https://deno.land/std/http/server.ts";
const server = serve({ port: 8000 });
console.log("http://localhost:8000/");
for await (const req of server) {
  req.respond({ body: "Hello World\n" });
}
```

### File Server

A small program for serving local files over HTTP

```sh
deno run --allow-net --allow-read https://deno.land/std/http/file_server.ts
> HTTP server listening on http://0.0.0.0:4500/
```

## Cookie

Helper to manipulate `Cookie` through `ServerRequest` and `Response`.

```ts
import { ServerRequest } from "https://deno.land/std/http/server.ts";
import { getCookies } from "https://deno.land/std/http/cookie.ts";

let request = new ServerRequest();
request.headers = new Headers();
request.headers.set("Cookie", "full=of; tasty=chocolate");

const cookies = getCookies(request);
console.log("cookies:", cookies);
// cookies: { full: "of", tasty: "chocolate" }
```

To set a `Cookie` you can add `CookieOptions` to properly set your `Cookie`

```ts
import { Response } from "https://deno.land/std/http/server.ts";
import { Cookie, setCookie } from "https://deno.land/std/http/cookie.ts";

let response: Response = {};
const cookie: Cookie = { name: "Space", value: "Cat" };
setCookie(response, cookie);

const cookieHeader = response.headers.get("set-cookie");
console.log("Set-Cookie:", cookieHeader);
// Set-Cookie: Space=Cat
```

Deleting a `Cookie` will set its expiration date before now. Forcing the browser
to delete it.

```ts
import { Response } from "https://deno.land/std/http/server.ts";
import { deleteCookie } from "https://deno.land/std/http/cookie.ts";

let response: Response = {};
deleteCookie(response, "deno");

const cookieHeader = response.headers.get("set-cookie");
console.log("Set-Cookie:", cookieHeader);
// Set-Cookie: deno=; Expires=Thus, 01 Jan 1970 00:00:00 GMT
```

**Note**: At the moment multiple `Set-Cookie` in a `Response` is not handled.
