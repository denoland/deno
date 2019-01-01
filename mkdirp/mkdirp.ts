import { ErrorKind, FileInfo, lstat, mkdir, platform } from "deno";

const PATH_SEPARATOR: string = platform.os === "win" ? "\\" : "/";

export async function mkdirp(path: string, mode?: number): Promise<void> {
  for (
    let parts: string[] = path.split(/\/|\\/),
      parts_len: number = parts.length,
      level: string,
      info: FileInfo,
      i: number = 0;
    i < parts_len;
    i++
  ) {
    level = parts.slice(0, i + 1).join(PATH_SEPARATOR);
    try {
      info = await lstat(level);
      if (!info.isDirectory()) throw Error(`${level} is not a directory`);
    } catch (err) {
      if (err.kind !== ErrorKind.NotFound) throw err;
      await mkdir(level, mode);
    }
  }
}
