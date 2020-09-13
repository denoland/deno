export type Payload =
  | { type: "open"; data: Event }
  | { type: "message"; data: MessageEvent }
  | { type: "close"; data: CloseEvent }
  | { type: "error"; data: Event | ErrorEvent };

export class AsyncWebSocket extends WebSocket {
  #readable: ReadableStream<Payload>;

  constructor(url: string, protocols?: string | string[]) {
    super(url, protocols);

    const { readable, writable } = new TransformStream<Payload, Payload>();
    this.#readable = readable;
    const writer = writable.getWriter();

    this.addEventListener("open", (e) => {
      writer.write({
        type: "open",
        data: e,
      });
    });
    this.addEventListener("message", (e) => {
      writer.write({
        type: "message",
        data: e,
      });
    });
    this.addEventListener("close", async (e) => {
      await writer.write({
        type: "close",
        data: e,
      });
      await writer.close();
    });
    this.addEventListener("error", (e) => {
      writer.write({
        type: "error",
        data: e,
      });
    });
  }

  async *[Symbol.asyncIterator]() {
    yield* this.#readable.getIterator();
  }
}
