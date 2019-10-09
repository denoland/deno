# http

A framework for creating HTTP/HTTPS server.

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

Deleting a `Cookie` will set its expiration date before now.
Forcing the browser to delete it.

```ts
import { Response } from "https://deno.land/std/http/server.ts";
import { delCookie } from "https://deno.land/std/http/cookie.ts";

let response: Response = {};
delCookie(response, "deno");

const cookieHeader = response.headers.get("set-cookie");
console.log("Set-Cookie:", cookieHeader);
// Set-Cookie: deno=; Expires=Thus, 01 Jan 1970 00:00:00 GMT
```

**Note**: At the moment multiple `Set-Cookie` in a `Response` is not handled.

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

Install it by using `deno install`

```sh
deno install file_server https://deno.land/std/http/file_server.ts --allow-net --allow-read
```
