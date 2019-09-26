// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { args, readFileSync, writeFileSync, exit } = Deno;

const name = args[1];
const test: (args: string[]) => void = {
  read(files: string[]): void {
    files.forEach(file => readFileSync(file));
  },
  write(files: string[]): void {
    files.forEach(file =>
      writeFileSync(file, new Uint8Array(0), { append: true })
    );
  },
  netFetch(hosts: string[]): void {
    hosts.forEach(host => fetch(host));
  },
  netListen(hosts: string[]): void {
    hosts.forEach(host => {
      const [hostname, port] = host.split(":");
      const listener = Deno.listen({ hostname, port: Number(port) });
      listener.close();
    });
  },
  async netDial(hosts: string[]): Promise<void> {
    for (const host of hosts) {
      const [hostname, port] = host.split(":");
      const listener = await Deno.dial({ hostname, port: Number(port) });
      listener.close();
    }
  }
}[name];

if (!test) {
  console.log("Unknown test:", name);
  exit(1);
}

test(args.slice(2));
