import {
  create,
  validate,
  setExpiration,
} from "./mod.ts";
import {
  Header,
  makeSignature,
} from "./create.ts";
import {
  checkHeaderCrit,
  Handlers,
  parse,
  isTokenObject,
} from "./validate.ts";

import { assertEquals, assertThrows } from "../testing/asserts.ts";
import { convertHexToBase64url } from "./_util.ts";

const key = "your-secret";

Deno.test("[jwt] makeSignature", async function (): Promise<void> {
  // https://www.freeformatter.com/hmac-generator.html
  const computedHmacInHex =
    "2b9e6619fa7f2c8d8b3565c88365376b75b1b0e5d87e41218066fd1986f2c056";
  const anotherVerifiedSignatureInBase64Url =
    "p2KneqJhji8T0PDlVxcG4DROyzTgWXbDhz_mcTVojXo";
  assertEquals(
    await makeSignature("HS256", "m$y-key", "thisTextWillBeEncrypted"),
    convertHexToBase64url(computedHmacInHex),
  );
  assertEquals(
    await makeSignature(
      "HS256",
      "m$y-key",
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ",
    ),
    anotherVerifiedSignatureInBase64Url,
  );
});

Deno.test("[jwt] isTokenObject", async function (): Promise<void> {
  const header:Header = {
    alg: "HS256",
    typ: "JWT",
  };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    iat: 1516239022,
  };
  const signature = "SARsBE5x_ua2ye823r2zKpQNaew3Daq8riKz5A4h3o4";
  const valid = isTokenObject({
    header,
    payload,
    signature,
  });
  assertEquals(valid, true);
});

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
  assertThrows(() => {
    parse(".aaa.bbb");
  }, Error);
  assertThrows((): void => {
    parse("a..aa.bbb");
  }, TypeError);
  assertThrows((): void => {
    parse("aaa.bbb.ccc.");
  }, Error);
  const jwt =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
  const header:Header = {
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
    await create({ header, payload, key: "your-256-bit-secret" }),
    jwt,
  );
});

Deno.test("[jwt] JSON Payload",
  async function (): Promise<void> {
    const header:Header = {
      alg: "HS256",
      typ: "JWT",
    };
    const payload = [3, 4, 5];
    const jwt = await create({ header, payload, key });
    const validatedPayload = await validate({ jwt, key, algorithm: "HS256" });
    assertEquals(
      jwt,
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.WzMsNCw1XQ.YlYdV_MrGWOv2Q_-9kpzjU2A1Payyg8gofvnYyUqz7M",
    );
    assertEquals(validatedPayload, payload);
  },
);

Deno.test("[jwt] expiredToken", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(-20000),
  };
  const header:Header = {
    alg: "HS256",
    dummy: 100,
  };
  const jwt = await create({ header, payload, key });

  try {
    await validate({ jwt, key, algorithm: "HS256" });
  } catch (error) {
    assertEquals(error.message, "the jwt is expired");
  }
});

Deno.test("[jwt] checkHeaderCrit", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(1),
  };
  const header:Header = {
    alg: "HS256",
    crit: ["dummy", "asyncDummy"],
    dummy: 100,
    asyncDummy: 200,
  };
  const critHandlers: Handlers = {
    dummy(value) {
      return value;
    },
    async asyncDummy(value) {
      return value;
    },
  };
  await checkHeaderCrit(header, critHandlers);
});

Deno.test("[jwt] critHandlers", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(1),
  };
  const header:Header = {
    alg: "HS256",
    crit: ["dummy"],
    dummy: 100,
  };
  const critHandlers = {
    dummy(value: any) {
      return value * 2;
    },
  };

  const jwt = await create({ header, payload, key });
  const validatedPayload = await validate({
    jwt,
    key,
    critHandlers,
    algorithm: ["HS256", "HS512"],
  });
  assertEquals(validatedPayload, payload);

  try {
    await validate({ jwt, key, algorithm: "HS256" });
  } catch (error) {
    assertEquals(
      error.message,
      "critical extension header parameters are not understood",
    );
  }
});

// https://tools.ietf.org/html/rfc7519#section-6
Deno.test("[jwt] none algorithm", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
  };
  const header: Header = {
    alg: "none",
    dummy: 100,
  };
  const jwt = await create({ header, payload, key });
  const validatedPayload = await validate({
    jwt,
    key: "keyIsIgnored",
    algorithm: "none",
  });
  assertEquals(validatedPayload, payload);
});

Deno.test("[jwt] HS256 algorithm", async function (): Promise<void> {
  const header:Header = {
    alg: "HS256",
    typ: "JWT",
  };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    iat: 1516239022,
  };
  const jwt = await create({ header, payload, key });
  const validatedPayload = await validate({ jwt, key, algorithm: "HS256" });
  assertEquals(
    jwt,
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SARsBE5x_ua2ye823r2zKpQNaew3Daq8riKz5A4h3o4",
  );
  assertEquals(validatedPayload, payload);
  try {
    const invalidJwt = // jwt with not supported crypto algorithm in alg header:
      "eyJhbGciOiJIUzM4NCIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTUxNjIzOTAyMn0.bQTnz6AuMJvmXXQsVPrxeQNvzDkimo7VNXxHeSBfClLufmCVZRUuyTwJF311JHuh";
    await validate({
      jwt: invalidJwt,
      key: "",
      algorithm: "HS256",
    });
  } catch (error) {
    assertEquals(error.message, "no matching algorithm: HS384");
  }
});

Deno.test("[jwt] HS512 algorithm", async function (): Promise<void> {
  const header:Header = { alg: "HS512", typ: "JWT" };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    admin: true,
    iat: 1516239022,
  };
  const key = "your-512-bit-secret";
  const externallyVerifiedJwt =
    "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTUxNjIzOTAyMn0.VFb0qJ1LRg_4ujbZoRMXnVkUgiuKq5KxWqNdbKq_G9Vvz-S1zZa9LPxtHWKa64zDl2ofkT8F6jBt_K4riU-fPg";
  const jwt = await create({ header, payload, key });
  const validatedPayload = await validate({ jwt, key, algorithm: "HS512" });
  assertEquals(validatedPayload, payload);
});