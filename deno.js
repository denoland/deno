import { bench, run } from "https://esm.sh/mitata";

const file = "/tmp/file.txt";

bench("writeTextFile()", async () => {
  await Deno.writeTextFile(file, "hello world");
});

run();
