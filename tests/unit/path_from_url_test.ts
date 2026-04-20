// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, assertThrows } from "./test_util.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { pathFromURL } = Deno[Deno.internal];

Deno.test(
  { ignore: Deno.build.os === "windows" },
  function pathFromURLPosix() {
    assertEquals(
      pathFromURL(new URL("file:///test/directory")),
      "/test/directory",
    );
    assertEquals(pathFromURL(new URL("file:///space_ .txt")), "/space_ .txt");
    assertThrows(() => pathFromURL(new URL("https://deno.land/welcome.ts")));
  },
);

Deno.test(
  { ignore: Deno.build.os !== "windows" },
  function pathFromURLWin32() {
    assertEquals(
      pathFromURL(new URL("file:///c:/windows/test")),
      "c:\\windows\\test",
    );
    assertEquals(
      pathFromURL(new URL("file:///c:/space_ .txt")),
      "c:\\space_ .txt",
    );
    assertEquals(
      pathFromURL(new URL("file:///D:/weird_names/ampersand_&.txt")),
      "D:\\weird_names\\ampersand_&.txt",
    );
    assertEquals(
      pathFromURL(new URL("file:///D:/weird_names/at_@.txt")),
      "D:\\weird_names\\at_@.txt",
    );
    assertEquals(
      pathFromURL(new URL("file:///D:/weird_names/emoji_%F0%9F%99%83.txt")),
      "D:\\weird_names\\emoji_🙃.txt",
    );
    assertEquals(
      pathFromURL(new URL("file:///D:/weird_names/percent_%25.txt")),
      "D:\\weird_names\\percent_%.txt",
    );
    assertEquals(
      pathFromURL(new URL("file:///D:/weird_names/pound_%23.txt")),
      "D:\\weird_names\\pound_#.txt",
    );
    assertEquals(
      pathFromURL(
        new URL(
          "file:///D:/weird_names/swapped_surrogate_pair_%EF%BF%BD%EF%BF%BD.txt",
        ),
      ),
      "D:\\weird_names\\swapped_surrogate_pair_\uFFFD\uFFFD.txt",
    );
    assertThrows(() => pathFromURL(new URL("https://deno.land/welcome.ts")));
  },
);
