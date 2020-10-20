# jwt

Create and verify JSON Web Tokens.

## JSON Web Token

### create

Takes a `payload`, `key` and `header` and returns the url-safe encoded `token`.

```typescript
import { create } from "https://deno.land/std@$STD_VERSION/jwt/mod.ts";

const payload = { foo: "bar" };
const key = "secret";

const token = await create(payload, key); // eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJmb28iOiJiYXIifQ.WePl7achkd0oGNB8XRF_LJwxlyiPZqpdNgdKpDboAjSTsWq-aOGNynTp8TOv8KjonFym8vwFwppXOLoLXbkIaQ
```

**Specific algorithm**

```typescript
const token = await create(payload, key, { header: { alg: "HS256" } });
```

### verify

Takes a `token`, `key` and an optional `options` object and returns the
`payload` of the `token` if the `token` is valid. Otherwise it throws an
`Error`.

```typescript
import { verify } from "https://deno.land/std@$STD_VERSION/jwt/mod.ts";

const token =
  "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJmb28iOiJiYXIifQ.WePl7achkd0oGNB8XRF_LJwxlyiPZqpdNgdKpDboAjSTsWq-aOGNynTp8TOv8KjonFym8vwFwppXOLoLXbkIaQ";
const key = "secret";

const payload = await verify(token, key); // { foo: "bar" }
```

**Specific algorithm**

```ts
const payload = await verify(token, key, { algorithm: "HS256" });
```

### decode

Takes a `token` to return an object with the `header`, `payload` and `signature`
properties if the `token` is valid. Otherwise it throws an `Error`.

```typescript
import { decode } from "https://deno.land/std@$STD_VERSION/jwt/mod.ts";

const token =
  "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJmb28iOiJiYXIifQ.WePl7achkd0oGNB8XRF_LJwxlyiPZqpdNgdKpDboAjSTsWq-aOGNynTp8TOv8KjonFym8vwFwppXOLoLXbkIaQ";

const { payload, signature, header } = await decode(token); // { header: { alg: "HS512", typ: "JWT" }, payload: { foo: "bar" }, signature: "59e3e5eda72191dd2818d07c5d117f2c9c3197288f66aa5d36074aa436e8023493b16abe68e18dca74e9f133aff0a8e89c5ca6f2fc05c29a5738ba0b5db90869" }
```

## Expiration

The optional **exp** claim in the payload (number of seconds since January 1,
1970, 00:00:00 UTC) that identifies the expiration time on or after which the
JWT must not be accepted for processing. This module checks if the current
date/time is before the expiration date/time listed in the **exp** claim.

```typescript
const oneHour = 60 * 60;
const token = await create({ exp: Date.now() + oneHour }, "secret");
```

## Algorithms

The following signature and MAC algorithms have been implemented:

- HS256 (HMAC SHA-256)
- HS512 (HMAC SHA-512)
- none ([_Unsecured JWTs_](https://tools.ietf.org/html/rfc7519#section-6)).

## Serialization

This application uses the JWS Compact Serialization only.

## Specifications

- [JSON Web Token](https://tools.ietf.org/html/rfc7519)
- [JSON Web Signature](https://www.rfc-editor.org/rfc/rfc7515.html)
- [JSON Web Algorithms](https://www.rfc-editor.org/rfc/rfc7518.html)
