// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals, assertThrows } from "./test_util.ts";

Deno.test(function cookieParseBasic() {
  const cookie = Deno.Cookie.parse(
    "session=abc123; Domain=example.com; Path=/sub; Max-Age=3600; Secure; HttpOnly; SameSite=Lax",
  );
  assertEquals(cookie.name, "session");
  assertEquals(cookie.value, "abc123");
  assertEquals(cookie.domain, "example.com");
  assertEquals(cookie.path, "/sub");
  assertEquals(cookie.maxAge, 3600);
  assertEquals(cookie.secure, true);
  assertEquals(cookie.httpOnly, true);
  assertEquals(cookie.sameSite, "Lax");
  assertEquals(cookie.partitioned, false);
});

Deno.test(function cookieParseExpires() {
  const cookie = Deno.Cookie.parse(
    "a=b; Expires=Sun, 06 Nov 1994 08:49:37 GMT",
  );
  assertEquals(cookie.expires, 784111777000);
});

Deno.test(function cookieParseInvalid() {
  assertThrows(() => Deno.Cookie.parse(""), TypeError);
  assertThrows(() => Deno.Cookie.parse("no-equals-sign"), TypeError);
  assertThrows(() => Deno.Cookie.parse("a=b\x00c"), TypeError);
});

Deno.test(function cookieSerializeRoundTrip() {
  const serialized = Deno.Cookie.serialize({
    name: "a",
    value: "b",
    domain: "example.com",
    path: "/",
    expires: new Date(784111777000),
    maxAge: 60,
    secure: true,
    httpOnly: true,
    sameSite: "Strict",
    partitioned: true,
  });
  assertEquals(
    serialized,
    "a=b; Domain=example.com; Path=/; Expires=Sun, 06 Nov 1994 08:49:37 GMT; Max-Age=60; Secure; HttpOnly; SameSite=Strict; Partitioned",
  );
  const parsed = Deno.Cookie.parse(serialized);
  assertEquals(parsed.name, "a");
  assertEquals(parsed.value, "b");
  assertEquals(parsed.expires, 784111777000);
});

Deno.test(function cookieSerializeInvalid() {
  assertThrows(() => Deno.Cookie.serialize({ name: "a b", value: "c" }));
  assertThrows(() => Deno.Cookie.serialize({ name: "a", value: "b;c" }));
  assertThrows(
    () => Deno.Cookie.serialize({ name: "a", value: "b", path: "relative" }),
  );
  assertThrows(
    () =>
      Deno.Cookie.serialize({
        name: "a",
        value: "b",
        // deno-lint-ignore no-explicit-any
        sameSite: "bogus" as any,
      }),
  );
});

Deno.test(function cookieJarBasics() {
  using jar = new Deno.CookieJar();
  jar.setCookie("a=1; Path=/", "https://example.com/");
  jar.setCookie({ name: "b", value: "2", domain: "example.com" });
  assertEquals(jar.getCookieString("https://example.com/"), "a=1; b=2");
  // Host-only cookie "a" is not sent to a subdomain, the domain cookie is.
  assertEquals(jar.getCookieString("https://www.example.com/"), "b=2");
  assertEquals(jar.getCookieString("https://other.com/"), null);

  const cookies = jar.getCookies("https://example.com/");
  assertEquals(cookies.length, 2);
  assertEquals(cookies[0].name, "a");
  assertEquals(cookies[0].hostOnly, true);

  assertEquals(jar.deleteCookie("a"), 1);
  assertEquals(jar.getCookieString("https://example.com/"), "b=2");
  jar.clear();
  assertEquals(jar.getCookies().length, 0);
});

Deno.test(function cookieJarSeedAndJson() {
  using jar = new Deno.CookieJar([
    { name: "a", value: "1", domain: "example.com" },
  ]);
  const exported = jar.toJSON();
  assertEquals(exported.length, 1);
  using jar2 = new Deno.CookieJar(exported);
  assertEquals(jar2.getCookieString("http://example.com/"), "a=1");
  // A cookie without a domain cannot be seeded.
  assertThrows(
    () => new Deno.CookieJar([{ name: "x", value: "y" }]),
    TypeError,
  );
});

Deno.test(function cookieJarExpiry() {
  using jar = new Deno.CookieJar();
  jar.setCookie("gone=1; Max-Age=0", "https://example.com/");
  assertEquals(jar.getCookieString("https://example.com/"), null);
  jar.setCookie("keep=1; Max-Age=10000", "https://example.com/");
  assertEquals(jar.getCookieString("https://example.com/"), "keep=1");
  // Setting Max-Age=0 deletes the stored cookie.
  jar.setCookie("keep=1; Max-Age=0", "https://example.com/");
  assertEquals(jar.getCookieString("https://example.com/"), null);
});

Deno.test(function cookieJarSecureRules() {
  using jar = new Deno.CookieJar();
  // Secure cookies cannot be stored from insecure origins.
  assertThrows(
    () => jar.setCookie("a=1; Secure", "http://example.com/"),
    TypeError,
  );
  jar.setCookie("a=1; Secure", "https://example.com/");
  // ... and are not sent over insecure connections.
  assertEquals(jar.getCookieString("http://example.com/"), null);
  assertEquals(jar.getCookieString("https://example.com/"), "a=1");
  // Loopback is treated as trustworthy.
  jar.setCookie("b=1; Secure", "http://localhost:8000/");
  assertEquals(jar.getCookieString("http://localhost:8000/"), "b=1");
});

Deno.test(
  { permissions: { net: true } },
  async function fetchCookieJarRoundTrip() {
    let requestCookies: string | null = "unset";
    const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
      const url = new URL(req.url);
      if (url.pathname === "/set") {
        return new Response("ok", {
          headers: [
            ["set-cookie", "session=secret123; Path=/; HttpOnly"],
            ["set-cookie", "theme=dark; Path=/"],
          ],
        });
      }
      requestCookies = req.headers.get("cookie");
      return new Response("ok");
    });
    const origin = `http://localhost:${server.addr.port}`;

    using client = Deno.createHttpClient({ cookieJar: true });
    assert(client.cookieJar instanceof Deno.CookieJar);

    const res1 = await fetch(`${origin}/set`, { client });
    await res1.body?.cancel();
    // Set-Cookie headers are still visible on the response.
    assertEquals(res1.headers.getSetCookie().length, 2);

    const res2 = await fetch(`${origin}/read`, { client });
    await res2.body?.cancel();
    assertEquals(requestCookies, "session=secret123; theme=dark");

    // An explicit Cookie header bypasses the jar.
    const res3 = await fetch(`${origin}/read`, {
      client,
      headers: { cookie: "manual=1" },
    });
    await res3.body?.cancel();
    assertEquals(requestCookies, "manual=1");

    // The jar is inspectable.
    const cookies = client.cookieJar!.getCookies(origin);
    assertEquals(cookies.map((c) => c.name).sort(), ["session", "theme"]);

    await server.shutdown();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchCookieJarRedirect() {
    let landingCookies: string | null = "unset";
    const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
      const url = new URL(req.url);
      if (url.pathname === "/redirect") {
        return new Response(null, {
          status: 302,
          headers: {
            "location": "/landing",
            "set-cookie": "hop=1; Path=/",
          },
        });
      }
      landingCookies = req.headers.get("cookie");
      return new Response("ok");
    });
    const origin = `http://localhost:${server.addr.port}`;

    using client = Deno.createHttpClient({ cookieJar: true });
    const res = await fetch(`${origin}/redirect`, { client });
    await res.body?.cancel();
    // The cookie set on the redirect hop is sent to the landing page.
    assertEquals(landingCookies, "hop=1");

    await server.shutdown();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchCookieJarShared() {
    let requestCookies: string | null = "unset";
    const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
      requestCookies = req.headers.get("cookie");
      return new Response("ok");
    });
    const origin = `http://localhost:${server.addr.port}`;

    using jar = new Deno.CookieJar();
    jar.setCookie("shared=1; Path=/", origin);

    using client1 = Deno.createHttpClient({ cookieJar: jar });
    using client2 = Deno.createHttpClient({ cookieJar: jar });
    assert(client1.cookieJar === jar);

    for (const client of [client1, client2]) {
      const res = await fetch(origin, { client });
      await res.body?.cancel();
      assertEquals(requestCookies, "shared=1");
    }

    // A client without a jar sends nothing.
    using client3 = Deno.createHttpClient({});
    const res = await fetch(origin, { client: client3 });
    await res.body?.cancel();
    assertEquals(requestCookies, null);

    await server.shutdown();
  },
);

Deno.test(function createHttpClientInvalidCookieJar() {
  assertThrows(
    // deno-lint-ignore no-explicit-any
    () => Deno.createHttpClient({ cookieJar: "yes" as any }),
    TypeError,
  );
});

Deno.test(function cookieMapBasics() {
  const map = new Deno.CookieMap("a=1; b=2; invalid; c=3");
  assertEquals(map.size, 3);
  assertEquals(map.get("a"), "1");
  assertEquals(map.get("missing"), undefined);
  assertEquals(map.has("b"), true);
  assertEquals(map.keys(), ["a", "b", "c"]);
  assertEquals(map.values(), ["1", "2", "3"]);
  assertEquals(map.entries(), [["a", "1"], ["b", "2"], ["c", "3"]]);
  assertEquals([...map], [["a", "1"], ["b", "2"], ["c", "3"]]);
  assertEquals(map.toString(), "a=1; b=2; c=3");
  assertEquals(map.toJSON(), { a: "1", b: "2", c: "3" });

  // First duplicate wins.
  const dupes = new Deno.CookieMap("a=1; a=2");
  assertEquals(dupes.get("a"), "1");

  const empty = new Deno.CookieMap();
  assertEquals(empty.size, 0);
});

Deno.test(function cookieMapFromHeaders() {
  const headers = new Headers({ cookie: "session=abc" });
  const map = new Deno.CookieMap(headers);
  assertEquals(map.get("session"), "abc");

  const noCookie = new Deno.CookieMap(new Headers());
  assertEquals(noCookie.size, 0);

  assertThrows(
    // deno-lint-ignore no-explicit-any
    () => new Deno.CookieMap(42 as any),
    TypeError,
  );
});

Deno.test(function cookieMapMutations() {
  const map = new Deno.CookieMap("a=1; b=2");
  map.set("theme", "dark", { path: "/", maxAge: 3600, httpOnly: true });
  assertEquals(map.get("theme"), "dark");
  assertEquals(map.delete("b"), true);
  assertEquals(map.delete("missing"), false);
  assertEquals(map.has("b"), false);

  const setCookies = map.toSetCookieStrings();
  assertEquals(setCookies.length, 3);
  assertEquals(setCookies[0], "theme=dark; Path=/; Max-Age=3600; HttpOnly");
  // Deletions serialize as immediately expiring cookies.
  assertEquals(
    setCookies[1],
    "b=; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Max-Age=0",
  );

  // Validation is eager.
  assertThrows(() => map.set("bad name", "x"), TypeError);
  assertThrows(() => map.set("name", "bad;value"), TypeError);
});

Deno.test(
  { permissions: { net: true } },
  async function cookieMapWithDenoServe() {
    const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
      const cookies = new Deno.CookieMap(req.headers);
      const count = Number(cookies.get("count") ?? "0") + 1;
      cookies.set("count", `${count}`, { path: "/" });
      const headers = new Headers();
      for (const setCookie of cookies.toSetCookieStrings()) {
        headers.append("set-cookie", setCookie);
      }
      return new Response(`${count}`, { headers });
    });
    const origin = `http://localhost:${server.addr.port}`;

    using client = Deno.createHttpClient({ cookieJar: true });
    assertEquals(await (await fetch(origin, { client })).text(), "1");
    assertEquals(await (await fetch(origin, { client })).text(), "2");
    assertEquals(await (await fetch(origin, { client })).text(), "3");

    await server.shutdown();
  },
);
