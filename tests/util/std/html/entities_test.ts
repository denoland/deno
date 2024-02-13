// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { escape, unescape } from "./entities.ts";
import { assertEquals } from "../assert/mod.ts";
import entityList from "./named_entity_list.json" assert { type: "json" };

Deno.test("escape", async (t) => {
  await t.step('escapes &<>"', () => {
    assertEquals(escape("&<>'\""), "&amp;&lt;&gt;&#39;&quot;");
  });
  await t.step("escapes ' to &#39; (not &apos;)", () => {
    assertEquals(escape("'"), "&#39;");
  });
  await t.step("doesn't escape non-breaking space", () => {
    assertEquals(escape("\xa0"), "\xa0");
  });
  await t.step(
    "doesn't escape other characters, even if they have named entities",
    () => {
      assertEquals(escape("þð"), "þð");
    },
  );
});

Deno.test("unescape", async (t) => {
  await t.step("round-trips with escape", () => {
    const chars = "&<>'\"";
    assertEquals(unescape(escape(chars)), chars);
  });

  await t.step("named entities", async (t) => {
    await t.step("default options", async (t) => {
      await t.step("unescapes &apos; as alias for ' &#39;", () => {
        assertEquals(unescape("&apos;"), "'");
      });
      await t.step("unescapes &nbsp;", () => {
        assertEquals(unescape("&nbsp;"), "\xa0");
      });
      await t.step("doesn't unescape other named entities", () => {
        assertEquals(unescape("&thorn;&eth;"), "&thorn;&eth;");
      });
    });

    await t.step("full entity list", async (t) => {
      await t.step("unescapes arbitrary named entities", () => {
        assertEquals(unescape("&thorn;&eth;", { entityList }), "þð");
      });
      await t.step(
        "unescapes truncated named entity (no trailing semicolon) if it is listed",
        () => {
          assertEquals(unescape("&amp", { entityList }), "&");
        },
      );
      await t.step(
        "consumes full named entity even when a truncated version is specified",
        () => {
          assertEquals(unescape("&amp;", { entityList }), "&");
        },
      );
      await t.step(
        "doesn't unescape truncated named entity if it isn't listed",
        () => {
          assertEquals(
            unescape("&therefore; &therefore", { entityList }),
            "∴ &therefore",
          );
        },
      );
    });
  });

  await t.step("decimal", async (t) => {
    await t.step("unescapes decimal", () => {
      assertEquals(unescape("&#46;"), ".");
    });
    await t.step("unescapes max decimal codepoint", () => {
      assertEquals(unescape("&#1114111;"), "\u{10ffff}");
    });
    await t.step("unescapes decimal with leading zero", () => {
      assertEquals(unescape("&#046;"), ".");
    });
    await t.step(
      "unescapes invalid decimal codepoint to replacement character",
      () => {
        assertEquals(unescape("&#1114112;"), "�");
      },
    );
  });

  await t.step("hex", async (t) => {
    await t.step("unescapes lower-case hex", () => {
      assertEquals(unescape("&#x2e;"), ".");
    });
    await t.step("unescapes upper-case hex", () => {
      assertEquals(unescape("&#x2E;"), ".");
    });
    await t.step("unescapes hex with leading zero", () => {
      assertEquals(unescape("&#x02E;"), ".");
    });
    await t.step("unescapes max hex codepoint", () => {
      assertEquals(unescape("&#x10ffff;"), "\u{10ffff}");
    });
    await t.step(
      "unescapes invalid hex codepoint to replacement character",
      () => {
        assertEquals(unescape("&#x110000;"), "�");
      },
    );
  });
});
