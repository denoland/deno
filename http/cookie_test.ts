// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { ServerRequest } from "./server.ts";
import { getCookie } from "./cookie.ts";
import { assertEquals } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";

test({
  name: "[HTTP] Cookie parser",
  fn(): void {
    let req = new ServerRequest();
    req.headers = new Headers();
    assertEquals(getCookie(req), {});
    req.headers = new Headers();
    req.headers.set("Cookie", "foo=bar");
    assertEquals(getCookie(req), { foo: "bar" });

    req.headers = new Headers();
    req.headers.set("Cookie", "full=of  ; tasty=chocolate");
    assertEquals(getCookie(req), { full: "of  ", tasty: "chocolate" });

    req.headers = new Headers();
    req.headers.set("Cookie", "igot=99; problems=but...");
    assertEquals(getCookie(req), { igot: "99", problems: "but..." });
  }
});
