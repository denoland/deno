import * as path from "./path/mod.ts";

/**
 * Test whether or not `dest` is a sub-directory of `src`
 * @param src src file path
 * @param dest dest file path
 * @param sep path separator
 */
export function isSubdir(
  src: string,
  dest: string,
  sep: string = path.sep
): boolean {
  if (src === dest) {
    return false;
  }
  const srcArray = src.split(sep);
  const destArray = dest.split(sep);

  return srcArray.reduce((acc, current, i) => {
    return acc && destArray[i] === current;
  }, true);
}

export enum PathType {
  file = "file",
  dir = "dir",
  symlink = "symlink"
}

/* Get a human readable file type string */
export function getFileInfoType(fileInfo: Deno.FileInfo): PathType | null {
  return fileInfo.isFile()
    ? PathType.file
    : fileInfo.isDirectory()
    ? PathType.dir
    : fileInfo.isSymlink()
    ? PathType.symlink
    : null;
}
