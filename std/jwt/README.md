[![nest badge](https://nest.land/badge.svg)](https://nest.land/package/djwt)

# djwt

The absolute minimum to make JSON Web Tokens in deno. Based on
[JWT](https://tools.ietf.org/html/rfc7519) and
[JWS](https://www.rfc-editor.org/rfc/rfc7515.html) specifications.

This library is accessible through the https://deno.land/x/ service and the 
https://nest.land/ service.

## Features

To generate JWTs which look in their finalized form like this (with line breaks
for display purposes only)

```
 eyJ0eXAiOiJKV1QiLA0KICJhbGciOiJIUzI1NiJ9
 .
 eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ
 .
 dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
```

... we use the mandatory
[**compact serialization**](https://www.rfc-editor.org/rfc/rfc7515.html#section-3.1)
process where a web token is represented as the concatenation of

`'BASE64URL(UTF8(JWS Protected Header))' || '.' || 'BASE64URL(JWS Payload)' ||'.'|| 'BASE64URL(JWS Signature)'`.

### Cryptographic Algorithm

The following signature and MAC algorithms - which are defined in the JSON Web
Algorithms (JWA) [specification](https://www.rfc-editor.org/rfc/rfc7518.html) -
have been implemented already: **HMAC SHA-256** ("HS256"), **HMAC SHA-512**
("HS512"), **RSASSA-PKCS1-v1_5 SHA-256** ("RS256") and **none**
([_Unsecured JWTs_](https://tools.ietf.org/html/rfc7519#section-6)).  
As soon as deno expands its
[crypto library](https://github.com/denoland/deno/tree/master/std/hash), we will
add more algorithms.

### Expiration Time

The optional **exp** claim identifies the expiration time on or after which the
JWT must not be accepted for processing. This library checks if the current
date/time is before the expiration date/time listed in the **exp** claim.

### Critical Header

This library supports the Critical Header Parameter **crit** which is described
in the JWS specification
[here](https://www.rfc-editor.org/rfc/rfc7515.html#section-4.1.11).

Look up
[this example](https://github.com/timonson/djwt/blob/master/examples/example.ts)
to see how the **crit** header parameter works.

## API

The API consists mostly of the two functions `makeJwt` and `validateJwt`,
generating and validating a JWT, respectively.

#### makeJwt({ key: string, header: Jose, payload: Payload }): Promise\<string>

The function `makeJwt` returns the url-safe encoded JWT as promise.

#### validateJwt({ jwt: string, key: string, algorithm: Algorithm | Algorithm[], critHandlers?: Handlers }): Promise\<JwtValidation>

The function `validateJwt` returns a _promise_. This promise resolves to an
_object_ with a _union type_ where the boolean property `isValid` serves as
[discriminant](https://www.typescriptlang.org/docs/handbook/advanced-types.html#discriminated-unions).  
If the JWT is valid (`.isValid === true`), the _type_ of the resolved promise
is:
`{ isValid: true; header: Jose; payload: Payload; signature: string; jwt: string; critResult?: unknown[] }`.  
If the JWT is invalid, the promise resolves to
`{ isValid: false; jwt: unknown; error: JwtError; isExpired: boolean }`.

The JWS specification [says](https://www.rfc-editor.org/rfc/rfc7515.html#page-8)
about the payload of a JWS the following:

> The payload can be any content and need not be a representation of a JSON
> object

Therefore, you must verify that the returned value is actually an object and has
the desired properties. Please take a look at
[this issue](https://github.com/timonson/djwt/issues/25) for more information.

#### setExpiration(exp: number | Date): number

Additionally there is the helper function `setExpiration` which simplifies
setting an expiration date. It takes either an `Date` object or a number (in
seconds) as argument.

```javascript
// A specific date:
setExpiration(new Date("2025-07-01"));
// One hour from now:
setExpiration(60 * 60);
```

## Example

Run the following _server_ example with `deno run -A example.ts`:

The server will respond to a **GET** request with a newly created JWT.  
On the other hand, if you send a JWT as data along with a **POST** request, the
server will check the validity of the JWT.

Always use [versioned imports](https://deno.land/x) for your dependencies. For
example `https://deno.land/x/djwt@v1.2/create.ts`.

```typescript
import { serve } from "https://deno.land/std/http/server.ts";
import { validateJwt } from "https://deno.land/x/djwt/validate.ts";
import { makeJwt, setExpiration, Jose, Payload } from "https://deno.land/x/djwt/create.ts";

const key = "your-secret";
const payload: Payload = {
  iss: "joe",
  exp: setExpiration(60),
};
const header: Jose = {
  alg: "HS256",
  typ: "JWT",
};

console.log("server is listening at 0.0.0.0:8000");
for await (const req of serve("0.0.0.0:8000")) {
  if (req.method === "GET") {
    req.respond({ body: (await makeJwt({ header, payload, key })) + "\n" });
  } else {
    const jwt = new TextDecoder().decode(await Deno.readAll(req.body));
    (await validateJwt({ jwt, key, algorithm: "HS256" })).isValid
      ? req.respond({ body: "Valid JWT\n" })
      : req.respond({ body: "Invalid JWT\n", status: 401 });
  }
}
```

## Applications

To see how djwt can be implemented further, you can find a djwt _middleware_
implementation for the [Oak](https://oakserver.github.io/oak/) framework
[here](https://github.com/halvardssm/oak-middleware-jwt).

## Contribution

Every kind of contribution to this project is highly appreciated.  
Please run `deno fmt` on the changed files before making a pull request.
