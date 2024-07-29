import { createReadStream } from "node:fs";
import path from "node:path";

const filePath = path.join(import.meta.dirname, "hello.txt");
const readableStream = createReadStream(filePath);
readableStream.on("data", (chunk) => {
  console.log(chunk.toString());
});
readableStream.on("end", () => {
  console.log("Finished reading the file");
});
readableStream.on("error", (error) => {
  console.error("Error reading the file:", error);
});
