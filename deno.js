import { bench, run } from "https://esm.sh/mitata";

const file = "/tmp/file.txt";

bench("writeTextFileSync()", () => {
  Deno.writeTextFileSync(file, "hello world");
})

run();
