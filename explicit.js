import { assertEquals } from "jsr:@std/assert";

{
  const filename = Deno.makeTempDirSync() + "/test_ftruncate.txt";
  using file = await Deno.open(filename, {
    create: true,
    read: true,
    write: true,
  });

  await file.truncate(20);
  assertEquals((await Deno.readFile(filename)).byteLength, 20);
  await file.truncate(5);
  assertEquals((await Deno.readFile(filename)).byteLength, 5);
  await file.truncate(-5);
  assertEquals((await Deno.readFile(filename)).byteLength, 0);

  await Deno.remove(filename);
}
