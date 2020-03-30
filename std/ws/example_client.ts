import {
  connectWebSocket,
  isWebSocketCloseEvent,
  isWebSocketPingEvent,
  isWebSocketPongEvent,
} from "../ws/mod.ts";
import { encode } from "../strings/mod.ts";
import { BufReader } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { blue, green, red, yellow } from "../fmt/colors.ts";

const endpoint = Deno.args[0] || "ws://127.0.0.1:8080";
/** simple websocket cli */
const sock = await connectWebSocket(endpoint);
console.log(green("ws connected! (type 'close' to quit)"));
(async function (): Promise<void> {
  for await (const msg of sock.receive()) {
    if (typeof msg === "string") {
      console.log(yellow("< " + msg));
    } else if (isWebSocketPingEvent(msg)) {
      console.log(blue("< ping"));
    } else if (isWebSocketPongEvent(msg)) {
      console.log(blue("< pong"));
    } else if (isWebSocketCloseEvent(msg)) {
      console.log(red(`closed: code=${msg.code}, reason=${msg.reason}`));
    }
  }
})();

const tpr = new TextProtoReader(new BufReader(Deno.stdin));
while (true) {
  await Deno.stdout.write(encode("> "));
  const line = await tpr.readLine();
  if (line === Deno.EOF) {
    break;
  }
  if (line === "close") {
    break;
  } else if (line === "ping") {
    await sock.ping();
  } else {
    await sock.send(line);
  }
  // FIXME: Without this,
  // sock.receive() won't resolved though it is readable...
  await new Promise((resolve): void => {
    setTimeout(resolve, 0);
  });
}
await sock.close(1000);
// FIXME: conn.close() won't shutdown process...
Deno.exit(0);
