// Copyright 2018-2026 the Deno authors. MIT license.
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test(async function responseText() {
  const response = new Response("hello world");
  const textPromise = response.text();
  assert(textPromise instanceof Promise);
  const text = await textPromise;
  assert(typeof text === "string");
  assertEquals(text, "hello world");
});

Deno.test(async function responseArrayBuffer() {
  const response = new Response(new Uint8Array([1, 2, 3]));
  const arrayBufferPromise = response.arrayBuffer();
  assert(arrayBufferPromise instanceof Promise);
  const arrayBuffer = await arrayBufferPromise;
  assert(arrayBuffer instanceof ArrayBuffer);
  assertEquals(new Uint8Array(arrayBuffer), new Uint8Array([1, 2, 3]));
});

Deno.test(async function responseJson() {
  const response = new Response('{"hello": "world"}');
  const jsonPromise = response.json();
  assert(jsonPromise instanceof Promise);
  const json = await jsonPromise;
  assert(json instanceof Object);
  assertEquals(json, { hello: "world" });
});

Deno.test(async function responseBlob() {
  const response = new Response(new Uint8Array([1, 2, 3]));
  const blobPromise = response.blob();
  assert(blobPromise instanceof Promise);
  const blob = await blobPromise;
  assert(blob instanceof Blob);
  assertEquals(blob.size, 3);
  assertEquals(await blob.arrayBuffer(), new Uint8Array([1, 2, 3]).buffer);
});

Deno.test(async function responseFormData() {
  const input = new FormData();
  input.append("hello", "world");
  const response = new Response(input);
  const contentType = response.headers.get("content-type")!;
  assert(contentType.startsWith("multipart/form-data"));
  const formDataPromise = response.formData();
  assert(formDataPromise instanceof Promise);
  const formData = await formDataPromise;
  assert(formData instanceof FormData);
  assertEquals([...formData], [...input]);
});

Deno.test(function responseInvalidInit() {
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  assertThrows(() => new Response("", 0));
  assertThrows(() => new Response("", { status: 0 }));
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  assertThrows(() => new Response("", { status: null }));
});

Deno.test(function responseNullInit() {
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  const response = new Response("", null);
  assertEquals(response.status, 200);
});

for (
  const [name, create] of [
    ["Response", (init: ResponseInit) => new Response(null, init)],
    ["Response.json", (init: ResponseInit) => Response.json(null, init)],
  ] as const
) {
  for (const proxy of [false, true]) {
    Deno.test(`${name} converts ResponseInit members in order (proxy: ${proxy})`, () => {
      const calls: string[] = [];
      const init = {
        get headers() {
          calls.push("headers");
          return {
            get "X-Test"() {
              calls.push("header value");
              return "value";
            },
          };
        },
        get status() {
          calls.push("status");
          return 200;
        },
        get statusText() {
          calls.push("statusText");
          return "OK";
        },
      };
      const response = create(proxy ? new Proxy(init, {}) : init);
      assertEquals(calls, ["headers", "header value", "status", "statusText"]);
      assertEquals(response.headers.get("x-test"), "value");
    });
  }

  Deno.test(`${name} converts headers before reading status`, () => {
    const calls: string[] = [];
    const headerError = new Error("header conversion");
    const init = {
      get headers() {
        calls.push("headers");
        return {
          get "X-Test"(): string {
            calls.push("header value");
            throw headerError;
          },
        };
      },
      get status(): number {
        calls.push("status");
        throw new Error("status getter");
      },
    };
    assertStrictEquals(assertThrows(() => create(init)), headerError);
    assertEquals(calls, ["headers", "header value"]);
  });
}

Deno.test(function responseJsonConvertsInitBeforeSerializingData() {
  const calls: string[] = [];
  const data = {
    toJSON() {
      calls.push("toJSON");
      return { ok: true };
    },
  };
  const init = {
    get headers() {
      calls.push("headers");
      return {};
    },
    get status() {
      calls.push("status");
      return 200;
    },
    get statusText() {
      calls.push("statusText");
      return "OK";
    },
  };
  Response.json(data, init);
  assertEquals(calls, ["headers", "status", "statusText", "toJSON"]);
});

Deno.test(function responseJsonInitErrorPrecedesSerialization() {
  const initError = new Error("init getter");
  const data = {
    toJSON() {
      throw new Error("toJSON");
    },
  };
  const init = {
    get headers(): HeadersInit {
      throw initError;
    },
  };
  assertStrictEquals(assertThrows(() => Response.json(data, init)), initError);
});

Deno.test(function customInspectFunction() {
  const response = new Response();
  assertEquals(
    Deno.inspect(response),
    `Response {
  body: null,
  bodyUsed: false,
  headers: Headers {},
  ok: true,
  redirected: false,
  status: 200,
  statusText: "",
  url: ""
}`,
  );
  assertStringIncludes(Deno.inspect(Response.prototype), "Response");
});

Deno.test(async function responseBodyUsed() {
  const response = new Response("body");
  assert(!response.bodyUsed);
  await response.text();
  assert(response.bodyUsed);
  // .body getter is needed so we can test the faulty code path
  response.body;
  assert(response.bodyUsed);
});
