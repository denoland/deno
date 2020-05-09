import {
  connectWebSocket,
  isWebSocketCloseEvent,
  isWebSocketPingEvent,
  isWebSocketPongEvent,
} from "./mod.ts";
import { encode } from "../encoding/utf8.ts";
import { BufReader } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { blue, green, red, yellow } from "../fmt/colors.ts";

if (import.meta.main) {
  const endpoint = Deno.args[0] || "ws://127.0.0.1:8080";
  /** simple websocket cli */
  try {
    const sock = await connectWebSocket(endpoint);
    console.log(green("ws connected! (type 'close' to quit)"));

    const messages = async (): Promise<void> => {
      for await (const msg of sock) {
        if (typeof msg === "string") {
          console.log(yellow(`< ${msg}`));
        } else if (isWebSocketPingEvent(msg)) {
          console.log(blue("< ping"));
        } else if (isWebSocketPongEvent(msg)) {
          console.log(blue("< pong"));
        } else if (isWebSocketCloseEvent(msg)) {
          console.log(red(`closed: code=${msg.code}, reason=${msg.reason}`));
        }
      }
    };

    const cli = async (): Promise<void> => {
      const tpr = new TextProtoReader(new BufReader(Deno.stdin));
      while (true) {
        await Deno.stdout.write(encode("> "));
        const line = await tpr.readLine();
        if (line === null) {
          break;
        }
        if (line === "close") {
          break;
        } else if (line === "ping") {
          await sock.ping();
        } else {
          await sock.send(line);
        }
      }
    };

    await Promise.race([messages(), cli()]).catch(console.error);

    if (!sock.isClosed) {
      await sock.close(1000).catch(console.error);
    }
  } catch (err) {
    console.error(red(`Could not connect to WebSocket: '${err}'`));
  }

  Deno.exit(0);
}
