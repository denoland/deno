// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { args, readFileSync, writeFileSync, exit, dial } = Deno;

const name = args[1];
const test: (args: string[]) => void = {
  read: (files: string[]): void => {
    files.forEach((file): any => readFileSync(file));
  },
  write: (files: string[]): void => {
    files.forEach(
      (file): any => writeFileSync(file, new Uint8Array(), { append: true })
    );
  },
  net_fetch: (hosts: string[]): void => {
    hosts.forEach((host): any => fetch(host));
  },
  net_listen: (hosts: string[]): void => {
    hosts.forEach(
      (host): any => {
        const listener = Deno.listen("tcp", host);
        listener.close();
      }
    );
  },
  net_dial: async (hosts: string[]): Promise<void> => {
    for (const host of hosts) {
      const listener = await Deno.dial("tcp", host);
      listener.close();
    }
  }
}[name];

if (!test) {
  console.log("Unknown test:", name);
  exit(1);
}

test(args.slice(2));
