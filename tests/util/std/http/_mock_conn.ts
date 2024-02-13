// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/** Create dummy `Deno.Conn` object with given base properties. */
export function mockConn(base: Partial<Deno.Conn> = {}): Deno.Conn {
  return {
    localAddr: {
      transport: "tcp",
      hostname: "",
      port: 0,
    },
    remoteAddr: {
      transport: "tcp",
      hostname: "",
      port: 0,
    },
    rid: -1,
    closeWrite: () => {
      return Promise.resolve();
    },
    read: (): Promise<number | null> => {
      return Promise.resolve(0);
    },
    write: (): Promise<number> => {
      return Promise.resolve(-1);
    },
    close: () => {},
    readable: new ReadableStream({
      type: "bytes",
      async pull(_controller) {
      },
      cancel() {
      },
      autoAllocateChunkSize: 1,
    }),
    writable: new WritableStream({
      async write(_chunk, _controller) {
      },
      close() {
      },
      abort() {
      },
    }),
    // TODO(ry) Remove the following ts-ignore.
    // @ts-ignore This was added to workaround incompatibilities between Deno versions.
    setNoDelay: (_nodelay?: boolean) => {},
    // @ts-ignore This was added to workaround incompatibilities between Deno versions.
    setKeepAlive: (_keepalive?: boolean) => {},
    ...base,
  };
}
