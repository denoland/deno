import { assertEquals, assertThrows } from "./test_util.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { pathFromURL } = Deno[Deno.internal];

Deno.test({
  name: "pathFromURLPosix",
  ignore: Deno.build.os === "windows",
  fn(): void {
    assertEquals(
      pathFromURL(new URL("file:///test/directory")),
      "/test/directory",
    );
    assertEquals(pathFromURL(new URL("file:///space_ .txt")), "/space_ .txt");
    assertThrows(() => pathFromURL(new URL("https://deno.land/welcome.ts")));
  },
});

Deno.test({
  name: "pathFromUrlWin32",
  ignore: Deno.build.os !== "windows",
  fn(): void {
    assertEquals(
      pathFromURL(new URL("file:///c:/windows/test")),
      "c:\\windows\\test",
    );
    assertEquals(
      pathFromURL(new URL("file:///c:/space_ .txt")),
      "c:\\space_ .txt",
    );
    assertThrows(() => pathFromURL(new URL("https://deno.land/welcome.ts")));
    /* TODO(ry) Add tests for these situations
     * ampersand_&.tx                 file:///D:/weird_names/ampersand_&.txt
     * at_@.txt                       file:///D:/weird_names/at_@.txt
     * emoji_🙃.txt                   file:///D:/weird_names/emoji_%F0%9F%99%83.txt
     * percent_%.txt                  file:///D:/weird_names/percent_%25.txt
     * pound_#.txt                    file:///D:/weird_names/pound_%23.txt
     * swapped_surrogate_pair_��.txt  file:///D:/weird_names/swapped_surrogate_pair_%EF%BF%BD%EF%BF%BD.txt
     */
  },
});
