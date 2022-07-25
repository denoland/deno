import { bench, run } from "mitata";
import { write } from "bun";

const file = "/tmp/file.txt";

bench("writeTextFileSync()", async () => {
  await write(file, "hello world");
})

await run();
