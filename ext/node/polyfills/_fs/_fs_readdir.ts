// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const { denoErrorToNodeError } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
import {
  type Dirent,
  getValidatedPathToString,
} from "ext:deno_node/internal/fs/utils.mjs";
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");
import { op_node_fs_readdir, op_node_fs_readdir_sync } from "ext:core/ops";

const {
  Error,
  PromisePrototypeThen,
} = primordials;

type readDirOptions = {
  encoding?: string;
  withFileTypes?: boolean;
  recursive?: boolean;
};

type readDirCallback = (err: Error | null, files: string[]) => void;

type readDirCallbackDirent = (err: Error | null, files: Dirent[]) => void;

type readDirBoth = (
  ...args: [Error] | [null, string[] | Dirent[] | Array<string | Dirent>]
) => void;

// Mirrors Node's lib/internal/fs/utils.js getOptions(): a bare string options
// arg is treated as { encoding: <string> }.
function normalizeOptions(
  options: readDirOptions | string | null | undefined,
): readDirOptions | null {
  if (typeof options === "string") {
    return { encoding: options };
  }
  return options ?? null;
}

function validateEncoding(encoding: string | undefined) {
  if (!encoding || encoding === "buffer") return;
  if (!Buffer.isEncoding(encoding)) {
    throw new Error(
      `TypeError [ERR_INVALID_OPT_VALUE_ENCODING]: The value "${encoding}" is invalid for option "encoding"`,
    );
  }
}

export function readdir(
  path: string | Buffer | URL,
  options: readDirOptions | string,
  callback: readDirCallback,
): void;
export function readdir(
  path: string | Buffer | URL,
  options: readDirOptions | string,
  callback: readDirCallbackDirent,
): void;
export function readdir(path: string | URL, callback: readDirCallback): void;
export function readdir(
  path: string | Buffer | URL,
  optionsOrCallback:
    | readDirOptions
    | string
    | readDirCallback
    | readDirCallbackDirent,
  maybeCallback?: readDirCallback | readDirCallbackDirent,
) {
  const callback =
    (typeof optionsOrCallback === "function"
      ? optionsOrCallback
      : maybeCallback) as readDirBoth | undefined;
  const options = normalizeOptions(
    typeof optionsOrCallback === "function" ? null : optionsOrCallback,
  );
  path = getValidatedPathToString(path);

  if (!callback) throw new Error("No callback function supplied");

  validateEncoding(options?.encoding);

  PromisePrototypeThen(
    op_node_fs_readdir(
      path,
      options?.recursive ?? false,
      options?.withFileTypes ?? false,
    ),
    (result) => callback(null, applyReaddirEncoding(result, options)),
    (err) => callback(denoErrorToNodeError(err, { syscall: "scandir", path })),
  );
}

// Names come back utf8 from the native op; re-encode for the rare non-utf8
// encodings (`buffer` -> Buffer name/parentPath; others -> re-encoded string).
function applyReaddirEncoding(
  result: Array<string | Dirent>,
  options: readDirOptions | null,
): Array<string | Dirent> {
  const enc = options?.encoding;
  if (!enc || enc === "utf8" || enc === "utf-8") return result;
  if (options?.withFileTypes) {
    for (let i = 0; i < result.length; i++) {
      const d = result[i] as Dirent;
      if (enc === "buffer") {
        d.name = Buffer.from(d.name as string, "utf8") as unknown as string;
        d.parentPath = Buffer.from(
          d.parentPath as string,
          "utf8",
        ) as unknown as string;
      } else {
        d.name = decode(d.name as string, enc) as string;
      }
    }
  } else {
    for (let i = 0; i < result.length; i++) {
      result[i] = decode(result[i] as string, enc);
    }
  }
  return result;
}

function decode(str: string, encoding?: string): string | Buffer {
  if (!encoding || encoding === "utf8" || encoding === "utf-8") {
    return str;
  }
  // "buffer" returns Buffer instances; every other (Node-supported) encoding
  // re-encodes the UTF-8 filename through Buffer to match Node's
  // lib/internal/fs/utils.js getDirent / readdir output.
  const buf = Buffer.from(str, "utf8");
  if (encoding === "buffer") return buf;
  // No primordial exists for Buffer.prototype.toString with an encoding.
  // deno-lint-ignore prefer-primordials
  return buf.toString(encoding as BufferEncoding);
}

export const readdirPromise = promisify(readdir) as (
  & ((path: string | Buffer | URL, options: {
    withFileTypes: true;
    encoding?: string;
  }) => Promise<Dirent[]>)
  & ((path: string | Buffer | URL, options?: {
    withFileTypes?: false;
    encoding?: string;
  }) => Promise<string[]>)
);

export function readdirSync(
  path: string | Buffer | URL,
  options: { withFileTypes: true; encoding?: string } | string,
): Dirent[];
export function readdirSync(
  path: string | Buffer | URL,
  options?: { withFileTypes?: false; encoding?: string } | string,
): string[];
export function readdirSync(
  path: string | Buffer | URL,
  rawOptions?: readDirOptions | string,
): Array<string | Dirent> {
  const options = normalizeOptions(rawOptions);
  path = getValidatedPathToString(path);

  validateEncoding(options?.encoding);

  // Native recursive walk + Dirent/name construction in Rust.
  let result: Array<string | Dirent>;
  try {
    result = op_node_fs_readdir_sync(
      path,
      options?.recursive ?? false,
      options?.withFileTypes ?? false,
    );
  } catch (e) {
    throw denoErrorToNodeError(e as Error, { syscall: "scandir", path });
  }

  return applyReaddirEncoding(result, options);
}
