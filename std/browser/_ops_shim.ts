// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { errors } from "https://raw.githubusercontent.com/denoland/deno/v1.1.0/cli/js/errors.ts";
export { errors } from "https://raw.githubusercontent.com/denoland/deno/v1.1.0/cli/js/errors.ts";

export enum SeekMode {
  Start = 0,
  Current = 1,
  End = 2,
}

class Resource {
  get buf(): ArrayBuffer {
    if (files.has(this.name)) {
      return files.get(this.name)!.buf;
    }
    const buf = new ArrayBuffer(0);
    files.set(this.name, { buf });
    return buf;
  }

  set buf(buf: ArrayBuffer) {
    if (files.has(this.name)) {
      files.get(this.name)!.buf = buf;
    } else {
      files.set(this.name, { buf });
    }
  }

  pos = 0;
  name: string;
  options?: Deno.OpenOptions;
  closed = false;

  constructor(name: string) {
    this.name = name;
  }
}

interface VirtualFile {
  buf: ArrayBuffer;
}

const files = new Map<string, VirtualFile>();

const resources = new Map<number, Resource>();
let resourceId = 0;

function closeResource(rid: number): void {
  if (resources.has(rid)) {
    resources.get(rid)!.closed = true;
    return;
  }
  throw new errors.BadResource(`Bad Resource: ${rid}`);
}

function copyResource(from: string, to: string): void {
  if (!files.has(from)) {
    throw new errors.NotFound(`File does not exist: ${from}`);
  }
  const toRid = openResource(to, { write: true, create: true });
  const data = new Uint8Array(files.get(from)!.buf);
  writeResource(toRid, data);
  closeResource(toRid);
}

export function getResources(): Deno.ResourceMap {
  const result: Deno.ResourceMap = {};
  for (const [key, value] of resources) {
    if (value.closed === false) {
      result[key] = value.name;
    }
  }
  return result;
}

function openResource(name: string, options: Deno.OpenOptions): number {
  const hasFile = files.has(name);
  if (hasFile) {
    if (options.createNew) {
      throw new errors.AlreadyExists(`File already exists: ${name}`);
    }
  }
  const rid = resourceId++;
  const resource = new Resource(name);
  resources.set(rid, resource);
  if (options.truncate) {
    new Uint8Array(resource.buf).set(new Uint8Array(0), 0);
  }
  resource.pos = options.append ? resource.buf.byteLength : 0;
  return rid;
}

/** De-reference any virtual files that are closed, so that their contents
 * can be garbage collected.  The contents of the virtual files will be lost,
 * if there are no other resources that have the file open. */
export function purgeResources(): void {
  const toDeleteResources: number[] = [];
  const toDeleteFiles: string[] = [];
  for (const [key, resource] of resources) {
    if (resource.closed) {
      toDeleteResources.push(key);
      if (!toDeleteFiles.includes(resource.name)) {
        toDeleteFiles.push(resource.name);
      }
    }
  }
  for (const key of toDeleteResources) {
    resources.delete(key);
  }
  for (const key of toDeleteFiles) {
    files.delete(key);
  }
}

function readResource(rid: number, data: Uint8Array): number {
  if (resources.has(rid)) {
    const item = resources.get(rid)!;
    if (item.closed) {
      throw new Error("rid closed");
    }
    if (item.options && !item.options.read) {
      throw new errors.PermissionDenied(
        `Resource not open for reading: ${rid}`
      );
    }
    const { pos } = item;
    const ab = new Uint8Array(item.buf);
    const remaining = ab.byteLength - pos;
    const readLength =
      remaining > data.byteLength ? data.byteLength : remaining;
    data.set(ab.slice(pos, pos + readLength), 0);
    item.pos += readLength;
    return readLength;
  }
  return -1;
}

function seekResource(rid: number, offset: number, whence: SeekMode): number {
  if (resources.has(rid)) {
    const item = resources.get(rid)!;
    if (item.closed) {
      throw new errors.BadResource(`Resource is closed: ${rid}`);
    }
    const ua = new Uint8Array(item.buf);
    switch (whence) {
      case SeekMode.Current:
        item.pos = item.pos + ua.byteLength;
        break;
      case SeekMode.End:
        item.pos = ua.byteLength - offset;
        break;
      case SeekMode.Start:
        item.pos;
    }
    if (item.pos >= ua.byteLength) {
      item.pos = ua.byteLength - 1;
    } else if (item.pos < 0) {
      item.pos = 0;
    }
    return item.pos;
  }
  throw new errors.BadResource(`Bad Resource: ${rid}`);
}

function truncateResource(path: string, len = 0): void {
  if (files.has(path)) {
    const item = files.get(path)!;
    const ab = new Uint8Array(item.buf);
    const nb = new Uint8Array(item.buf.slice(0, len));
    ab.set(nb, 0);
  } else {
    throw new errors.NotFound(`File not found: ${path}`);
  }
}

function writeResource(rid: number, data: Uint8Array): number {
  if (resources.has(rid)) {
    const item = resources.get(rid)!;
    if (item.closed) {
      throw new errors.BadResource(`Resource is closed: ${rid}`);
    }
    if (item.options && !item.options.write) {
      throw new errors.PermissionDenied(
        `Resource not open for writing: ${rid}`
      );
    }
    const ab = new Uint8Array(item.buf);
    const byteLength = data.byteLength + item.pos;
    const b = new Uint8Array(
      ab.byteLength > byteLength ? ab.byteLength : byteLength
    );
    b.set(ab, 0);
    b.set(data, item.pos);
    item.buf = b;
    item.pos += data.byteLength;
    return data.byteLength;
  }
  return -1;
}

// ops

export function close(rid: number): void {
  closeResource(rid);
}

export function copyFile(fromPath: string, toPath: string): void {
  copyResource(fromPath, toPath);
}

export function open(path: string, options: Deno.OpenOptions): number {
  return openResource(path, options);
}

// eslint-disable-next-line require-await
export async function read(
  rid: number,
  buffer: Uint8Array
): Promise<number | null> {
  if (buffer.length === 0) {
    return Promise.resolve(0);
  }
  const nread = readResource(rid, buffer);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread === 0) {
    return null;
  } else {
    return nread;
  }
}

export function readSync(rid: number, buffer: Uint8Array): number | null {
  if (buffer.length === 0) {
    return 0;
  }
  const nread = readResource(rid, buffer);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread === 0) {
    return null;
  } else {
    return nread;
  }
}

export function seek(
  rid: number,
  offset: number,
  whence: SeekMode
): Promise<number> {
  try {
    const result = seekResource(rid, offset, whence);
    return Promise.resolve(result);
  } catch (e) {
    return Promise.reject(e);
  }
}

export function seekSync(
  rid: number,
  offset: number,
  whence: SeekMode
): number {
  return seekResource(rid, offset, whence);
}

export function truncate(path: string, len?: number): void {
  truncateResource(path, len);
}

// eslint-disable-next-line require-await
export async function write(rid: number, data: Uint8Array): Promise<number> {
  const result = writeResource(rid, data);
  if (result < 0) {
    throw new Error("write error");
  } else {
    return result;
  }
}

export function writeSync(rid: number, data: Uint8Array): number {
  const result = writeResource(rid, data);
  if (result < 0) {
    throw new Error("write error");
  }
  return result;
}
