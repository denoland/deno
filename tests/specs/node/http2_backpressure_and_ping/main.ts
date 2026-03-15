import assert from "node:assert/strict";
import { once } from "node:events";
import * as http2 from "node:http2";
import net from "node:net";

function withTimeout<T>(
  promise: Promise<T>,
  label: string,
  ms = 5_000,
): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`${label} timed out`)), ms)
    ),
  ]);
}

const server = http2.createServer();
server.on("stream", (stream) => {
  stream.respond({
    [http2.constants.HTTP2_HEADER_STATUS]: http2.constants.HTTP_STATUS_OK,
  });
  stream.end("ok");
});

await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
const port = (server.address() as { port: number }).port;
const authority = `http://127.0.0.1:${port}`;

interface BackpressureState {
  writesWhileBlocked: number;
  injectedBackpressure: boolean;
  drainReleased: boolean;
}

function createBackpressureState(): BackpressureState {
  return {
    writesWhileBlocked: 0,
    injectedBackpressure: false,
    drainReleased: false,
  };
}

function createInstrumentedConnection(
  state: BackpressureState,
  options: { autoReleaseDrain: boolean },
) {
  const socket = net.connect({ host: "127.0.0.1", port });
  const originalWrite = socket.write.bind(socket);
  socket.write = ((chunk: unknown, ...args: unknown[]) => {
    if (state.injectedBackpressure && !state.drainReleased) {
      state.writesWhileBlocked++;
    }
    const ret = originalWrite(chunk as never, ...(args as never[]));
    if (!state.injectedBackpressure) {
      state.injectedBackpressure = true;
      if (options.autoReleaseDrain) {
        process.nextTick(() => {
          state.drainReleased = true;
          socket.emit("drain");
        });
      }
      return false;
    }
    return ret;
  }) as typeof socket.write;
  return socket;
}

try {
  const backpressureState = createBackpressureState();
  const client = http2.connect(authority, {
    createConnection: () =>
      createInstrumentedConnection(backpressureState, {
        autoReleaseDrain: true,
      }),
  });
  await withTimeout(once(client, "remoteSettings"), "client remoteSettings");

  let pingSync = true;
  let pingCallbackWasSync = true;
  await withTimeout(
    new Promise<void>((resolve, reject) => {
      const sent = client.ping((error, duration, payload) => {
        try {
          pingCallbackWasSync = pingSync;
          assert.equal(error, null);
          assert.equal(payload.length, 8);
          assert.equal(duration >= 0, true);
          resolve();
        } catch (e) {
          reject(e);
        }
      });
      assert.equal(sent, true);
      pingSync = false;
    }),
    "client ping",
  );

  for (let i = 0; i < 32; i++) {
    const req = client.request({
      [http2.constants.HTTP2_HEADER_METHOD]: http2.constants.HTTP2_METHOD_GET,
      [http2.constants.HTTP2_HEADER_PATH]: "/",
    });
    req.resume();
    await withTimeout(once(req, "end"), `request end ${i}`);
  }

  client.close();
  await withTimeout(once(client, "close"), "client close");

  assert.equal(backpressureState.injectedBackpressure, true);
  assert.equal(backpressureState.writesWhileBlocked, 0);
  assert.equal(pingCallbackWasSync, false);

  for (let i = 0; i < 20; i++) {
    const raceState = createBackpressureState();
    const closeRaceClient = http2.connect(authority, {
      createConnection: () =>
        createInstrumentedConnection(raceState, { autoReleaseDrain: true }),
    });
    await withTimeout(
      once(closeRaceClient, "remoteSettings"),
      `closeRace remoteSettings ${i}`,
    );

    let closeRaceSync = true;
    let closeRaceCallbackWasSync = true;
    await withTimeout(
      new Promise<void>((resolve) => {
        const sent = closeRaceClient.ping(() => {
          closeRaceCallbackWasSync = closeRaceSync;
          resolve();
        });
        assert.equal(sent, true);
        closeRaceClient.close();
        closeRaceSync = false;
      }),
      `closeRace ping callback ${i}`,
    );
    await withTimeout(once(closeRaceClient, "close"), `closeRace close ${i}`);
    assert.equal(closeRaceCallbackWasSync, false);
  }

  // Close with pending writes while transport waits on drain.
  const stuckDrainState = createBackpressureState();
  let stuckDrainSocket: net.Socket | null = null;
  const stuckDrainClient = http2.connect(authority, {
    createConnection: () => {
      stuckDrainSocket = createInstrumentedConnection(stuckDrainState, {
        autoReleaseDrain: false,
      });
      return stuckDrainSocket;
    },
  });
  await withTimeout(
    once(stuckDrainClient, "remoteSettings"),
    "stuckDrain remoteSettings",
  );
  for (let i = 0; i < 16; i++) {
    const req = stuckDrainClient.request({
      [http2.constants.HTTP2_HEADER_METHOD]: http2.constants.HTTP2_METHOD_GET,
      [http2.constants.HTTP2_HEADER_PATH]: "/",
    });
    req.on("error", () => {});
    req.end();
    req.resume();
  }
  stuckDrainClient.close();
  setTimeout(() => {
    stuckDrainSocket?.destroy();
  }, 50);
  await withTimeout(once(stuckDrainClient, "close"), "stuckDrain close", 7_500);
  assert.equal(stuckDrainState.injectedBackpressure, true);

  console.log("HTTP2_BACKPRESSURE_PING_OK");
} finally {
  server.close();
}
