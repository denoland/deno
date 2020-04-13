// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { args, readFileSync, writeFileSync, exit } = Deno;

const name = args[0];
const test: { [key: string]: Function } = {
  read(files: string[]): void {
    files.forEach((file) => readFileSync(file));
  },
  write(files: string[]): void {
    files.forEach((file) =>
      writeFileSync(file, new Uint8Array(0), { append: true })
    );
  },
  netFetch(hosts: string[]): void {
    hosts.forEach((host) => fetch(host));
  },
  netListen(endpoints: string[]): void {
    endpoints.forEach((endpoint) => {
      const [hostname, port] = endpoint.split(":");
      const listener = Deno.listen({
        transport: "tcp",
        hostname,
        port: parseInt(port, 10),
      });
      listener.close();
    });
  },
  async netConnect(endpoints: string[]): Promise<void> {
    for (const endpoint of endpoints) {
      const [hostname, port] = endpoint.split(":");
      const listener = await Deno.connect({
        transport: "tcp",
        hostname,
        port: parseInt(port, 10),
      });
      listener.close();
    }
  },
};

if (!test[name]) {
  console.log("Unknown test:", name);
  exit(1);
}

test[name](args.slice(1));
