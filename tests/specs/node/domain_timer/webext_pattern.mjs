// Reproduction of the web-ext rdp-client.js connect pattern from #27037.
// web-ext uses domain.create() to wrap net.createConnection so that
// ECONNREFUSED errors reject the promise instead of crashing the process.
import net from "node:net";
import EventEmitter from "node:events";
import domain from "node:domain";

class FirefoxRDPClient extends EventEmitter {
  _rdpConnection;
  _active = new Map();

  connect(port) {
    return new Promise((resolve, reject) => {
      const d = domain.create();
      d.once("error", reject);
      d.run(() => {
        const conn = net.createConnection({ port, host: "127.0.0.1" });
        this._rdpConnection = conn;
        conn.on("error", (...args) => this.onError(...args));
        conn.on("data", () => {});
        conn.on("end", () => {});
        this._active.set("root", { resolve, reject });
      });
    });
  }

  onError(error) {
    this.emit("error", error);
  }
}

async function connectWithRetries(port, maxRetries = 2) {
  for (let i = 0; i <= maxRetries; i++) {
    try {
      const client = new FirefoxRDPClient();
      return await client.connect(port);
    } catch (error) {
      if (error.code === "ECONNREFUSED") {
        console.log(`Retry ${i}: connection refused`);
        await new Promise((r) => setTimeout(r, 50));
      } else {
        throw error;
      }
    }
  }
  console.log("All retries exhausted (expected)");
}

await connectWithRetries(19999, 2);
console.log("ok - web-ext retry pattern works");
