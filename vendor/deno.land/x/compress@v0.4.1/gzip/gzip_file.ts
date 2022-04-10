/** very fast */
import { gunzip, gzip } from "../zlib/mod.ts";
/** slow */
// import { gzip, gunzip } from "./gzip.ts";
import { readAll, writeAll } from "../deps.ts";

export async function gzipFile(src: string, dest: string): Promise<void> {
  const reader = await Deno.open(src, {
    read: true,
  });
  const writer = await Deno.open(dest, {
    write: true,
    create: true,
    truncate: true,
  });
  await writeAll(writer, gzip(await readAll(reader), undefined));
  writer.close();
  reader.close();
}

export async function gunzipFile(src: string, dest: string): Promise<void> {
  const reader = await Deno.open(src, {
    read: true,
  });
  const writer = await Deno.open(dest, {
    write: true,
    create: true,
    truncate: true,
  });
  await writeAll(writer, gunzip(await readAll(reader)));
}
