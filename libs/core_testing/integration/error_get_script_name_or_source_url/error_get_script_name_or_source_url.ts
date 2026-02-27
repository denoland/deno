// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore no-explicit-any
(Error as any).prepareStackTrace = (_err: unknown, frames: any[]) => {
  return frames.map((frame) => frame.getScriptNameOrSourceURL());
};

new Promise((_, reject) => {
  reject(new Error("fail").stack);
}).catch((err) => {
  console.log(err);
});
