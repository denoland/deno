import { assertThrows, assertEquals, unitTest } from "./test_util.ts";

const {
  pathFromURL,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

unitTest(
  { ignore: Deno.build.os === "windows", perms: {} },
  function pathFromURLPosix(): void {
    assertEquals(pathFromURL("file:///test/directory"), "/test/directory");
    assertThrows(() => pathFromURL("file://host/test/directory"));
  }
);

unitTest(
  { ignore: Deno.build.os !== "windows", perms: {} },
  function pathFromURLWin32(): void {
    assertEquals(pathFromURL("file:///c:/windows/test"), "c:\\windows\\test");
    assertThrows(() => pathFromURL("file:///thing/test"));
  }
);
