#!/usr/bin/env -S deno run --allow-read

for (const item of Deno.readDirSync(".")) {
  console.log(item.name);
}
