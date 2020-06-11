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
  }
);
