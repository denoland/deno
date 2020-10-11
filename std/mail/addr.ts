import { parseAddr as wasmParseAddr } from "./_wasm/mail.ts";

interface MailAddrList {
  displayName?: string;
  addr?: string;
  groupName?: string;
  addrs?: MailAddrList[];
}

export function parseAddr(addr: string): MailAddrList[] {
  return wasmParseAddr(addr);
}
