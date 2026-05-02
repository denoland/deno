import { createWriteStream } from "node:fs";
const outputWriteStream = createWriteStream("./env.js", {
  encoding: "utf-8",
});

outputWriteStream.write("something", "utf-8");
