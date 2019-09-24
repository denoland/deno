// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
async function main(): Promise<void> {
  const res = await fetch("http://deno.land/welcome.ts");
  console.log(`Response http: ${await res.text()}`);
}

main();
