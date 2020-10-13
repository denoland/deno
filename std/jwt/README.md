# jwt

Make JSON Web Tokens in deno. Based on
[JWT](https://tools.ietf.org/html/rfc7519),
[JWS](https://www.rfc-editor.org/rfc/rfc7515.html) and [JWA](https://www.rfc-editor.org/rfc/rfc7518.html) specifications.

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
("HS512") and **none** ([_Unsecured JWTs_](https://tools.ietf.org/html/rfc7519#section-6)).

### Expiration Time

The optional **exp** claim identifies the expiration time on or after which the
JWT must not be accepted for processing. This library checks if the current
date/time is before the expiration date/time listed in the **exp** claim.

## Usage

The API consists mostly of the two functions `create` and `validate`, generating
and validating a JWT, respectively.

### create

Takes a `payload`, `key` and `header` to return the url-safe encoded JWT as promise.

```typescript
import { create } from 'https://deno.land/std/jwt/mod.ts'

create()
.
.
.

```

### verify

Takes a `jwt`, `key` and an object with a the property `algorithm` to return the `payload` of the `jwt` as `promise`, if the `jwt` is valid. Otherwise it throws an `Error`.

```typescript
import { verify } from 'https://deno.land/std/jwt/mod.ts'

verify()
.
.
.
```

### setExpiration

Takes either an `Date` object or a `number` (in seconds) as argument and returns the number of seconds since January 1, 1970, 00:00:00 UTC

```typescript
import { setExpiration } from 'https://deno.land/std/jwt/mod.ts'

// A specific date:
setExpiration(new Date("2025-07-01"));
// One hour from now:
setExpiration(60 * 60);
```

### decode

Takes a `jwt` to return an object with the `header`, `payload` and `signature` properties.

```typescript
import { decode } from 'https://deno.land/std/jwt/mod.ts'

decode()
.
.
.
```
