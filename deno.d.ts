declare module "deno" {
  type MessageCallback = (msg: Uint8Array) => void;
  function sub(channel: string, cb: MessageCallback): void;
  function pub(channel: string, payload: Uint8Array): null | ArrayBuffer;
}
