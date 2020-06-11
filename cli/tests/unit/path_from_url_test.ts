import { assertThrows, assertEquals, unitTest } from "./test_util.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { pathFromURL } = Deno[Deno.internal];

unitTest(
  { ignore: Deno.build.os === "windows" },
  function pathFromURLPosix(): void {
    assertEquals(pathFromURL("file:///test/directory"), "/test/directory");
    assertThrows(() => pathFromURL("file://host/test/directory"));
    assertThrows(() => pathFromURL("https://deno.land/welcome.ts"));
  }
);

unitTest(
  { ignore: Deno.build.os !== "windows" },
  function pathFromURLWin32(): void {
    assertEquals(pathFromURL("file:///c:/windows/test"), "c:\\windows\\test");
    assertThrows(() => pathFromURL("file:///thing/test"));
    assertThrows(() => pathFromURL("https://deno.land/welcome.ts"));
    /* TODO(ry) Add tests for these situations
     * ampersand_&.tx                 file:///D:/weird_names/ampersand_&.txt
     * at_@.txt                       file:///D:/weird_names/at_@.txt
     * emoji_🙃.txt                   file:///D:/weird_names/emoji_%F0%9F%99%83.txt
     * percent_%.txt                  file:///D:/weird_names/percent_%25.txt
     * pound_#.txt                    file:///D:/weird_names/pound_%23.txt
     * space_ .txt                    file:///D:/weird_names/space_%20.txt
     * swapped_surrogate_pair_��.txt  file:///D:/weird_names/swapped_surrogate_pair_%EF%BF%BD%EF%BF%BD.txt
     */
  }
);
