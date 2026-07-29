import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Pipe, constants: PipeConstants, createPipe } = require(
  "internal/test/binding",
).internalBinding("pipe_wrap");

const [readFd, writeFd] = createPipe();
const worker = new Worker(
  new URL("./isolate_fd_ownership_worker.ts", import.meta.url),
  { type: "module", deno: { permissions: "none" } },
);

const workerResult = await new Promise<number>((resolve, reject) => {
  worker.onmessage = (event) => resolve(event.data);
  worker.onerror = (event) => reject(event.error);
  worker.postMessage(readFd);
});
worker.terminate();

if (workerResult === 0) {
  throw new Error("worker adopted a descriptor owned by another isolate");
}
console.log("PASS: worker rejected an unregistered descriptor");

const readPipe = new Pipe(PipeConstants.SOCKET);
if (readPipe.open(readFd) !== 0) {
  throw new Error("owner could not adopt its registered descriptor");
}
console.log("PASS: owner adopted its registered descriptor");

const duplicatePipe = new Pipe(PipeConstants.SOCKET);
if (duplicatePipe.open(readFd) === 0) {
  throw new Error("descriptor was adopted more than once");
}
console.log("PASS: descriptor could only be adopted once");

const writePipe = new Pipe(PipeConstants.SOCKET);
if (writePipe.open(writeFd) !== 0) {
  throw new Error("owner could not adopt the other pipe descriptor");
}

await Promise.all([
  new Promise<void>((resolve) => readPipe.close(resolve)),
  new Promise<void>((resolve) => writePipe.close(resolve)),
]);
