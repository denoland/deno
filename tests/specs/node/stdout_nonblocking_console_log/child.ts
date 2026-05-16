const libc = Deno.dlopen("libc.so.6", {
  fcntl: { parameters: ["i32", "i32", "i32"], result: "i32" },
});

const F_GETFL = 3;
const F_SETFL = 4;
const O_NONBLOCK = 2048;

const flags = libc.symbols.fcntl(1, F_GETFL, 0);
if (flags < 0) {
  throw new Error("F_GETFL failed");
}
if (libc.symbols.fcntl(1, F_SETFL, flags | O_NONBLOCK) < 0) {
  throw new Error("F_SETFL failed");
}

try {
  const chunk = new Uint8Array(16 * 1024);
  chunk.fill("x".charCodeAt(0));
  while (true) {
    try {
      Deno.stdout.writeSync(chunk);
    } catch (error) {
      if (error instanceof Deno.errors.WouldBlock) {
        break;
      }
      throw error;
    }
  }
  console.log("ok");
  console.error("ok");
} finally {
  libc.close();
}
