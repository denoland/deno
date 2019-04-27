# http

A framework for creating HTTP/HTTPS server.

## Cookie

Helper to manipulate `Cookie` throught `ServerRequest` and `Response`.

```ts
import { getCookies } from "https://deno.land/std/http/cookie.ts";

let req = new ServerRequest();
req.headers = new Headers();
req.headers.set("Cookie", "full=of; tasty=chocolate");

const c = getCookies(request);
// c = { full: "of", tasty: "chocolate" }
```

To set a `Cookie` you can add `CookieOptions` to properly set your `Cookie`

```ts
import { setCookie } from "https://deno.land/std/http/cookie.ts";

let res: Response = {};
res.headers = new Headers();
setCookie(res, { name: "Space", value: "Cat" });
```

Deleting a `Cookie` will set its expiration date before now.
Forcing the browser to delete it.

```ts
import { delCookie } from "https://deno.land/std/http/cookie.ts";

let res = new Response();
delCookie(res, "deno");
// Will append this header in the response
// "Set-Cookie: deno=; Expires=Thus, 01 Jan 1970 00:00:00 GMT"
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

Add the following to your `.bash_profile`

```
alias file_server="deno --allow-net https://deno.land/std/http/file_server.ts"
```
