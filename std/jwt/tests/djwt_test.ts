import {
  makeJwt,
  makeSignature,
  convertHexToBase64url,
  setExpiration,
} from "../create.ts";
import {
  validateJwt,
  validateJwtObject,
  checkHeaderCrit,
  parseAndDecode,
  isExpired,
  Handlers,
} from "../validate.ts";
import {
  convertBase64urlToBase64,
  convertBase64ToBase64url,
} from "../base64/base64url.ts";
import {
  convertBase64ToUint8Array,
  convertUint8ArrayToBase64,
} from "../base64/base64.ts";
import {
  assertEquals,
  assertThrows,
  convertUint8ArrayToHex,
  convertHexToUint8Array,
  dirname,
  fromFileUrl,
} from "./test_deps.ts";

const key = "your-secret";

Deno.test("makeSetAndCheckExpirationTest", function (): void {
  // A specific date:
  const t1 = setExpiration(new Date("2020-01-01"));
  const t2 = setExpiration(new Date("2099-01-01"));
  // Ten seconds from now:
  const t3 = setExpiration(10);
  // One hour from now:
  const t4 = setExpiration(60 * 60);
  //  1 second from now:
  const t5 = setExpiration(1);
  //  1 second earlier:
  const t6 = setExpiration(-1);
  assertEquals(isExpired(t1), true);
  assertEquals(isExpired(t2), false);
  assertEquals(10, t3 - Math.round(Date.now() / 1000));
  assertEquals(isExpired(t4), false);
  assertEquals(isExpired(t5), false);
  assertEquals(isExpired(t6), true);
  // add leeway:
  assertEquals(isExpired(t6, 1500), false);
  assertEquals(setExpiration(10), setExpiration(new Date(Date.now() + 10000)));
});

Deno.test("makeDataConversionTest", function (): void {
  const hex1 =
    "a4a99a8e21149ccbc5c5aabd310e5d5208b12db90dff749171d5014b688ce808";
  const hex2 = convertUint8ArrayToHex(
    convertBase64ToUint8Array(
      convertBase64urlToBase64(
        convertBase64ToBase64url(
          convertUint8ArrayToBase64(
            convertHexToUint8Array(
              convertUint8ArrayToHex(
                convertBase64ToUint8Array(
                  convertBase64urlToBase64(convertHexToBase64url(hex1)),
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  assertEquals(hex1, hex2);
});

Deno.test("makeSignatureTests", async function (): Promise<void> {
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

Deno.test("makeValidateJwtObjectTest", async function (): Promise<void> {
  const header = {
    alg: "HS256" as const,
    typ: "JWT",
  };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    iat: 1516239022,
  };
  const signature = "SARsBE5x_ua2ye823r2zKpQNaew3Daq8riKz5A4h3o4";
  const jwtObject = validateJwtObject({
    header,
    payload,
    signature,
  });
  assertEquals(jwtObject.payload, payload);
  assertThrows(
    (): void => {
      const jwtObject = validateJwtObject({
        header: {
          alg: 10,
          typ: "JWT",
        },
        payload,
        signature,
      });
    },
    ReferenceError,
    "header parameter 'alg' is not a string",
  );
});

Deno.test("parseAndDecodeTests", async function (): Promise<void> {
  assertEquals(
    parseAndDecode(
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.TVCeFl1nnZWUMQkAQKuSo_I97YeIZAS8T1gOkErT7F8",
    ),
    {
      header: { alg: "HS256", typ: "JWT" },
      payload: {},
      signature:
        "4d509e165d679d959431090040ab92a3f23ded87886404bc4f580e904ad3ec5f",
    },
  );
  assertThrows((): void => {
    parseAndDecode(".aaa.bbb");
  }, SyntaxError);

  assertThrows((): void => {
    parseAndDecode(".aaa.bbb");
  }, SyntaxError);
  assertThrows((): void => {
    parseAndDecode("a..aa.bbb");
  }, TypeError);
  assertThrows((): void => {
    parseAndDecode("aaa.bbb.ccc.");
  }, SyntaxError);
  const jwt =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
  const header = {
    alg: "HS256" as const,
    typ: "JWT",
  };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    iat: 1516239022,
  };
  assertEquals(parseAndDecode(jwt), {
    header,
    payload,
    signature:
      "49f94ac7044948c78a285d904f87f0a4c7897f7e8f3a4eb2255fda750b2cc397",
  });
  assertEquals(
    await makeJwt({ header, payload, key: "your-256-bit-secret" }),
    jwt,
  );
});

Deno.test("makeCreationAndValidationTest", async function (): Promise<void> {
  const header = {
    alg: "HS256" as const,
    typ: "JWT",
  };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    iat: 1516239022,
  };
  const jwt = await makeJwt({ header, payload, key });
  const validatedJwt = await validateJwt({ jwt, key, algorithm: "HS256" });
  assertEquals(
    jwt,
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SARsBE5x_ua2ye823r2zKpQNaew3Daq8riKz5A4h3o4",
  );
  if (validatedJwt.isValid) {
    assertEquals(validatedJwt.payload, payload);
    assertEquals(validatedJwt.header, header);
    assertEquals(
      jwt.slice(jwt.lastIndexOf(".") + 1),
      convertHexToBase64url(validatedJwt!.signature),
    );
  } else {
    throw new Error("invalid JWT");
  }
  const invalidJwt = // jwt with not supported crypto algorithm in alg header:
    "eyJhbGciOiJIUzM4NCIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTUxNjIzOTAyMn0.bQTnz6AuMJvmXXQsVPrxeQNvzDkimo7VNXxHeSBfClLufmCVZRUuyTwJF311JHuh";
  const invalidatedJwt = await validateJwt({
    jwt: invalidJwt,
    key: "",
    algorithm: "HS256",
  });
  if (invalidatedJwt.isValid) throw Error("jwt should be invalid");
  else {
    assertEquals(invalidatedJwt.error.message, "no matching algorithm: HS384");
  }
});

Deno.test(
  "makeCreationAndValidationTestWithOtherJsonPayload",
  async function (): Promise<void> {
    const header = {
      alg: "HS256" as const,
      typ: "JWT",
    };
    const payload = [3, 4, 5];
    const jwt = await makeJwt({ header, payload, key });
    const validatedJwt = await validateJwt({ jwt, key, algorithm: "HS256" });
    assertEquals(
      jwt,
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.WzMsNCw1XQ.YlYdV_MrGWOv2Q_-9kpzjU2A1Payyg8gofvnYyUqz7M",
    );
    if (validatedJwt.isValid) {
      assertEquals(validatedJwt.payload, payload);
      assertEquals(validatedJwt.header, header);
      assertEquals(
        jwt.slice(jwt.lastIndexOf(".") + 1),
        convertHexToBase64url(validatedJwt!.signature),
      );
    } else {
      throw new Error("invalid JWT");
    }
  },
);

Deno.test("testExpiredJwt", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(-20000),
  };
  const header = {
    alg: "HS256" as const,
    dummy: 100,
  };
  const jwt = await makeJwt({ header, payload, key });

  const validatedJwt = await validateJwt({ jwt, key, algorithm: "HS256" });
  if (validatedJwt.isValid) throw Error("jwt should be invalid");
  else {
    assertEquals(validatedJwt.error.message, "the jwt is expired");
    assertEquals(validatedJwt.isExpired, true);
  }
});

Deno.test("makeCheckHeaderCritTest", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(1),
  };
  const header = {
    alg: "HS256" as const,
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
  const result = await checkHeaderCrit(header, critHandlers);
  assertEquals(result, [100, 200]);
});

Deno.test("makeHeaderCritTest", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(1),
  };
  const header = {
    alg: "HS256" as const,
    crit: ["dummy"],
    dummy: 100,
  };
  const critHandlers = {
    dummy(value: any) {
      return value * 2;
    },
  };

  const jwt = await makeJwt({ header, payload, key });
  const validatedJwt = await validateJwt({
    jwt,
    key,
    critHandlers,
    algorithm: ["HS256", "HS512"],
  });
  if (validatedJwt.isValid) {
    assertEquals(validatedJwt.critResult, [200]);
    assertEquals(validatedJwt.payload, payload);
    assertEquals(validatedJwt.header, header);
    assertEquals(
      jwt.slice(jwt.lastIndexOf(".") + 1),
      convertHexToBase64url(validatedJwt!.signature),
    );
  } else {
    throw new Error("invalid JWT");
  }

  const failing = await validateJwt({ jwt, key, algorithm: "HS256" });
  if (failing.isValid) throw Error("jwt should be invalid");
  else {
    assertEquals(
      failing.error.message,
      "critical extension header parameters are not understood",
    );
    assertEquals(failing.isExpired, false);
  }
});

// https://tools.ietf.org/html/rfc7519#section-6
Deno.test("makeUnsecuredJwtTest", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
  };
  const header = {
    alg: "none" as const,
    dummy: 100,
  };
  const jwt = await makeJwt({ header, payload, key });
  const validatedJwt = await validateJwt({
    jwt,
    key: "keyIsIgnored",
    algorithm: "none",
  });
  if (validatedJwt.isValid) {
    assertEquals(validatedJwt.payload, payload);
    assertEquals(validatedJwt.header, header);
    assertEquals(validatedJwt.signature, "");
  } else {
    throw new Error("invalid JWT");
  }
});

Deno.test("makeHmacSha512Test", async function (): Promise<void> {
  const header = { alg: "HS512" as const, typ: "JWT" };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    admin: true,
    iat: 1516239022,
  };
  const key = "your-512-bit-secret";
  const externallyVerifiedJwt =
    "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTUxNjIzOTAyMn0.VFb0qJ1LRg_4ujbZoRMXnVkUgiuKq5KxWqNdbKq_G9Vvz-S1zZa9LPxtHWKa64zDl2ofkT8F6jBt_K4riU-fPg";
  const jwt = await makeJwt({ header, payload, key });
  const validatedJwt = await validateJwt({ jwt, key, algorithm: "HS512" });
  if (validatedJwt.isValid) {
    assertEquals(jwt, externallyVerifiedJwt);
    assertEquals(validatedJwt.payload, payload);
    assertEquals(validatedJwt.header, header);
  } else {
    throw new Error("invalid JWT");
  }
});

Deno.test("makeRS256Test", async function (): Promise<void> {
  const header = { alg: "RS256" as const, typ: "JWT" };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    admin: true,
    iat: 1516239022,
  };
  const moduleDir = dirname(fromFileUrl(import.meta.url));
  const publicKey = Deno.readTextFileSync(moduleDir + "/public.pem");
  const privateKey = Deno.readTextFileSync(moduleDir + "/private.pem");
  const externallyVerifiedJwt =
    "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTUxNjIzOTAyMn0.POstGetfAytaZS82wHcjoTyoqhMyxXiWdR7Nn7A29DNSl0EiXLdwJ6xC6AfgZWF1bOsS_TuYI3OG85AmiExREkrS6tDfTQ2B3WXlrr-wp5AokiRbz3_oB4OxG-W9KcEEbDRcZc0nH3L7LzYptiy1PtAylQGxHTWZXtGz4ht0bAecBgmpdgXMguEIcoqPJ1n3pIWk_dUZegpqx0Lka21H6XxUTxiy8OcaarA8zdnPUnV6AmNP3ecFawIFYdvJB_cm-GvpCSbr8G8y_Mllj8f4x9nBH8pQux89_6gUY618iYv7tuPWBFfEbLxtF2pZS6YC1aSfLQxeNe8djT9YjpvRZA";
  const jwt = await makeJwt({ header, payload, key: privateKey });
  const validatedJwt = await validateJwt({
    jwt,
    key: publicKey,
    algorithm: "RS256",
  });
  if (validatedJwt.isValid) {
    assertEquals(jwt, externallyVerifiedJwt);
    assertEquals(validatedJwt.payload, payload);
    assertEquals(validatedJwt.header, header);
  } else {
    throw new Error("invalid JWT");
  }
});
