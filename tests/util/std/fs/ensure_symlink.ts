// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { dirname } from "../path/dirname.ts";
import { resolve } from "../path/resolve.ts";
import { ensureDir, ensureDirSync } from "./ensure_dir.ts";
import { getFileInfoType, toPathString } from "./_util.ts";

const isWindows = Deno.build.os === "windows";

function resolveSymlinkTarget(target: string | URL, linkName: string | URL) {
  if (typeof target !== "string") return target; // URL is always absolute path
  if (typeof linkName === "string") {
    return resolve(dirname(linkName), target);
  } else {
    return new URL(target, linkName);
  }
}

/**
 * Ensures that the link exists, and points to a valid file.
 * If the directory structure does not exist, it is created.
 *
 * @param target the source file path
 * @param linkName the destination link path
 */
export async function ensureSymlink(
  target: string | URL,
  linkName: string | URL,
) {
  const targetRealPath = resolveSymlinkTarget(target, linkName);
  const srcStatInfo = await Deno.lstat(targetRealPath);
  const srcFilePathType = getFileInfoType(srcStatInfo);

  await ensureDir(dirname(toPathString(linkName)));

  const options: Deno.SymlinkOptions | undefined = isWindows
    ? {
      type: srcFilePathType === "dir" ? "dir" : "file",
    }
    : undefined;

  try {
    await Deno.symlink(target, linkName, options);
  } catch (error) {
    if (!(error instanceof Deno.errors.AlreadyExists)) {
      throw error;
    }
  }
}

/**
 * Ensures that the link exists, and points to a valid file.
 * If the directory structure does not exist, it is created.
 *
 * @param target the source file path
 * @param linkName the destination link path
 */
export function ensureSymlinkSync(
  target: string | URL,
  linkName: string | URL,
) {
  const targetRealPath = resolveSymlinkTarget(target, linkName);
  const srcStatInfo = Deno.lstatSync(targetRealPath);
  const srcFilePathType = getFileInfoType(srcStatInfo);

  ensureDirSync(dirname(toPathString(linkName)));

  const options: Deno.SymlinkOptions | undefined = isWindows
    ? {
      type: srcFilePathType === "dir" ? "dir" : "file",
    }
    : undefined;

  try {
    Deno.symlinkSync(target, linkName, options);
  } catch (error) {
    if (!(error instanceof Deno.errors.AlreadyExists)) {
      throw error;
    }
  }
}
