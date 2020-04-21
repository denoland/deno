/** Create dummy Deno.Conn object with given base properties */
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
    closeRead: (): void => {},
    closeWrite: (): void => {},
    read: (): Promise<number | Deno.EOF> => {
      return Promise.resolve(0);
    },
    // eslint-disable-next-line require-await
    async *[Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array> {
      yield new Uint8Array();
    },
    write: (): Promise<number> => {
      return Promise.resolve(-1);
    },
    close: (): void => {},
    ...base,
  };
}
