import { assertEquals } from "../testing/asserts.ts";
import * as url from "./url.ts";

Deno.test({
  name: "[url] URL",
  fn() {
    assertEquals(url.URL, URL);
  },
});
