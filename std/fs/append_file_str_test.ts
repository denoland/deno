import { assertEquals } from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { appendFileStr, appendFileStrSync } from "./append_file_str.ts";

const testdataDir = path.resolve("fs", "testdata");

Deno.test("testReadFileSync", function (): void {
  const jsonFile = path.join(testdataDir, "append_file_1.json");
  const content1 = "append_file_str_test_1";
  const content2 = "append_file_str_test_2";

  appendFileStrSync(jsonFile, content1);
  appendFileStrSync(jsonFile, content2);

  // make sure file have been create.
  Deno.statSync(jsonFile);

  const result = new TextDecoder().decode(Deno.readFileSync(jsonFile));

  // remove test file
  Deno.removeSync(jsonFile);

  assertEquals(content1 + content2, result);
});

Deno.test("testReadFile", async function (): Promise<void> {
  const jsonFile = path.join(testdataDir, "append_file_2.json");
  const content1 = "append_file_str_test_1";
  const content2 = "append_file_str_test_2";
  
  await appendFileStr(jsonFile, content1);
  await appendFileStr(jsonFile, content2);

  // make sure file have been create.
  await Deno.stat(jsonFile);

  const result = new TextDecoder().decode(await Deno.readFile(jsonFile));

  // remove test file
  await Deno.remove(jsonFile);

  assertEquals(content1 + content2, result);
});
