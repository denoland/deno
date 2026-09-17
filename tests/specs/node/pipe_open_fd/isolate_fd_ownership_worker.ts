import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Pipe, constants: PipeConstants } = require("internal/test/binding")
  .internalBinding("pipe_wrap");

globalThis.onmessage = (event) => {
  const pipe = new Pipe(PipeConstants.SOCKET);
  const result = pipe.open(event.data);
  globalThis.postMessage(result);
  if (result === 0) {
    pipe.close(() => globalThis.close());
  } else {
    globalThis.close();
  }
};
