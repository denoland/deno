// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const curl = async (url_: string) => {
  const res = await fetch(url_);
  await Deno.copy(Deno.stdout, res.body);
}

const usage = () => {
  console.log('Usage:');
  console.log('   curl --allow-net [url]');
}

Deno.args.length ? curl(Deno.args[0]) : usage();
