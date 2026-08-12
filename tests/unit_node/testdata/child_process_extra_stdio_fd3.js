import { createWriteStream } from "node:fs";

const stream = createWriteStream(null, { fd: 3 });

stream.on("error", (err) => {
  console.error(err);
  process.exit(1);
});
stream.on("close", () => {
  process.exit(0);
});

stream.end("hello from fd 3");
