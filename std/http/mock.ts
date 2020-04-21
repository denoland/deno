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
    next(): Promise<IteratorResult<Uint8Array>> {
      return Promise.resolve({ done: true, value: undefined });
    },
    [Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array> {
      return this;
    },
    write: (): Promise<number> => {
      return Promise.resolve(-1);
    },
    close: (): void => {},
    ...base,
  };
}
