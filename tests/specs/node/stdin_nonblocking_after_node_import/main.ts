// Importing node:process triggers process.stdin lazy init, which
// calls uv_pipe_open(0) and sets O_NONBLOCK on fd 0. After this,
// Deno.stdin.read() must still work (retry on WouldBlock internally).
import "node:process";

const buf = new Uint8Array(1024);
const n = await Deno.stdin.read(buf);
if (n === null) {
  console.log("got null");
} else {
  console.log(new TextDecoder().decode(buf.subarray(0, n)));
}
