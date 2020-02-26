/** Create dummy Deno.Conn object with given base properties */
export function mockConn(base: Partial<Deno.Conn> = {}): Deno.Conn {
  return {
    localAddr: {
      transport: "tcp",
      hostname: "",
      port: 0
    },
    remoteAddr: {
      transport: "tcp",
      hostname: "",
      port: 0
    },
    rid: -1,
    closeRead: (): void => {},
    closeWrite: (): void => {},
    read: async (): Promise<number | Deno.EOF> => {
      return 0;
    },
    write: async (): Promise<number> => {
      return -1;
    },
    close: (): void => {},
    ...base
  };
}
