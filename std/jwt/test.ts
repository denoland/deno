import {
  create,
  Header,
  isTokenObject,
  parse,
  setExpiration,
  verify,
} from "./mod.ts";

import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "../testing/asserts.ts";

const header: Header = {
  alg: "HS256",
  typ: "JWT",
};
const payload = {
  name: "John Doe",
};
const signature = "abc";
const exp = setExpiration(new Date("2035-07-01"));

Deno.test("[jwt] isTokenObject", function (): void {
  assert(isTokenObject({ header, payload, signature }));
  assert(
    isTokenObject({ header, payload: "payloadAsString", signature }),
  );
  assert(
    isTokenObject(
      {
        header,
        payload: { exp },
        signature,
      },
    ),
  );

  // @ts-ignore */
  assertEquals(isTokenObject("invalid"), false);
  // @ts-ignore */
  assertEquals(isTokenObject({ header: "invalid" }), false);
  // @ts-ignore */
  assertEquals(isTokenObject({ signature: 123 }), false);
});

Deno.test("[jwt] verify", async function (): Promise<void> {
  const key = "secret";

  await assertThrowsAsync(
    async () => {
      // @ts-ignore */
      const jwt = await create("", key, { header: "invalid" });
      verify(jwt, key, { algorithm: "HS512" });
    },
    Error,
  );

  await assertThrowsAsync(
    async () => {
      // @ts-ignore */
      const jwt = await create("", key, { header: { alg: "invalidAlg" } });
      await verify(jwt, key, { algorithm: "HS512" });
    },
    Error,
  );
  await assertThrowsAsync(async () => {
    // @ts-ignore */
    const jwt = await create({ header: {}, payload });
    await verify(jwt, key, { algorithm: "HS512" });
  }, Error);
  await assertThrowsAsync(
    async () => {
      // @ts-ignore */
      const jwt = await create({ header, payload });
      await verify(jwt, key, { algorithm: "HS512" });
    },
    Error,
  );
  await assertThrowsAsync(
    async () => {
      // @ts-ignore */
      const jwt = await create({ exp: 100 }, key, { header });
      await verify(jwt, key, { algorithm: "HS512" });
    },
    Error,
    "jwt is expired",
  );
});

Deno.test("[jwt] setExpiration", function (): void {
  assertEquals(setExpiration(10), setExpiration(new Date(Date.now() + 10000)));
});

const key = "your-secret";

Deno.test("[jwt] parse", async function (): Promise<void> {
  assertEquals(
    parse(
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.TVCeFl1nnZWUMQkAQKuSo_I97YeIZAS8T1gOkErT7F8",
    ),
    {
      header: { alg: "HS256", typ: "JWT" },
      payload: {},
      signature:
        "4d509e165d679d959431090040ab92a3f23ded87886404bc4f580e904ad3ec5f",
    },
  );
  // "ImEi" === base64url("a")
  assertThrows(() => {
    // SyntaxError: Unexpected end of JSON input
    parse("aaa");
  }, SyntaxError);
  assertThrows(() => {
    // SyntaxError: Unexpected end of JSON input
    parse("ImEi..ImEi");
  }, SyntaxError);
  assertThrows((): void => {
    // TypeError: Illegal base64url string!
    parse("a");
  }, TypeError);
  assertThrows((): void => {
    // TypeError: invalid serialization
    parse("ImEi.ImEi.ImEi.ImEi");
  }, TypeError);

  const jwt =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
  const header: Header = {
    alg: "HS256",
    typ: "JWT",
  };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    iat: 1516239022,
  };
  assertEquals(parse(jwt), {
    header,
    payload,
    signature:
      "49f94ac7044948c78a285d904f87f0a4c7897f7e8f3a4eb2255fda750b2cc397",
  });
  assertEquals(
    await create(payload, "your-256-bit-secret", { header }),
    jwt,
  );
});

Deno.test("[jwt] expired token", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(-20000),
  };
  const header: Header = {
    alg: "HS256",
    dummy: 100,
  };
  const jwt = await create(payload, key, { header });

  try {
    await verify(jwt, key, { algorithm: "HS256" });
  } catch (error) {
    assertEquals(error.message, "jwt is expired");
  }
});

Deno.test("[jwt] none algorithm", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
  };
  const header: Header = {
    alg: "none",
    dummy: 100,
  };
  const jwt = await create(payload, key, { header });
  const validatedPayload = await verify(
    jwt,
    "keyIsIgnored",
    {
      algorithm: "none",
    },
  );
  assertEquals(validatedPayload, payload);
});

Deno.test("[jwt] HS256 algorithm", async function (): Promise<void> {
  const header: Header = {
    alg: "HS256",
    typ: "JWT",
  };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    iat: 1516239022,
  };
  const jwt = await create(payload, key, { header });
  const validatedPayload = await verify(jwt, key, { algorithm: "HS256" });
  assertEquals(
    jwt,
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SARsBE5x_ua2ye823r2zKpQNaew3Daq8riKz5A4h3o4",
  );
  assertEquals(validatedPayload, payload);
  try {
    const invalidJwt = // jwt with not supported crypto algorithm in alg header:
      "eyJhbGciOiJIUzM4NCIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTUxNjIzOTAyMn0.bQTnz6AuMJvmXXQsVPrxeQNvzDkimo7VNXxHeSBfClLufmCVZRUuyTwJF311JHuh";
    await verify(invalidJwt, "", {
      algorithm: "HS256",
    });
  } catch (error) {
    assertEquals(error.message, "algorithms do not match");
  }
});

Deno.test("[jwt] HS512 algorithm", async function (): Promise<void> {
  const header: Header = { alg: "HS512", typ: "JWT" };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    admin: true,
    iat: 1516239022,
  };
  const key = "your-512-bit-secret";
  const jwt = await create(payload, key, { header });
  const validatedPayload = await verify(jwt, key, { algorithm: "HS512" });
  assertEquals(validatedPayload, payload);
});
