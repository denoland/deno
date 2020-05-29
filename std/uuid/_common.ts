// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
export function bytesToUuid(bytes: number[] | Uint8Array): string {
  const bits: string[] = [...bytes].map((bit): string => {
    const s: string = bit.toString(16);
    return bit < 0x10 ? "0" + s : s;
  });
  return [
    ...bits.slice(0, 4),
    "-",
    ...bits.slice(4, 6),
    "-",
    ...bits.slice(6, 8),
    "-",
    ...bits.slice(8, 10),
    "-",
    ...bits.slice(10, 16),
  ].join("");
}

export function uuidToBytes(uuid: string): number[] {
  const bytes: number[] = [];

  uuid.replace(/[a-fA-F0-9]{2}/g, (hex: string): string => {
    bytes.push(parseInt(hex, 16));
    return "";
  });

  return bytes;
}

export function stringToBytes(str: string): number[] {
  str = unescape(encodeURIComponent(str));
  const bytes = new Array(str.length);
  for (let i = 0; i < str.length; i++) {
    bytes[i] = str.charCodeAt(i);
  }
  return bytes;
}

export function createBuffer(content: number[]): ArrayBuffer {
  const arrayBuffer = new ArrayBuffer(content.length);
  const uint8Array = new Uint8Array(arrayBuffer);
  for (let i = 0; i < content.length; i++) {
    uint8Array[i] = content[i];
  }
  return arrayBuffer;
}
