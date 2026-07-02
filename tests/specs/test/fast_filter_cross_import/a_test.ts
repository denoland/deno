// Importing another test file: its `Deno.test()` calls run while this module
// evaluates, so its tests register under this file too.
import "./b_test.ts";

Deno.test("other", () => {});
