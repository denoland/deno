// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const name = Deno.args[0];
const test: { [key: string]: Function } = {
  read(files: string[]): void {
    files.forEach((file) => Deno.readFileSync(file));
  },
  write(files: string[]): void {
    files.forEach((file) =>
      Deno.writeFileSync(file, new Uint8Array(0), { append: true })
    );
  },
  netFetch(hosts: string[]): void {
    hosts.forEach((host) => fetch(host));
  },
  netListen(endpoints: string[]): void {
    endpoints.forEach((endpoint) => {
      const index = endpoint.lastIndexOf(":");
      const [hostname, port] = [
        endpoint.substr(0, index),
        endpoint.substr(index + 1),
      ];
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
      const index = endpoint.lastIndexOf(":");
      const [hostname, port] = [
        endpoint.substr(0, index),
        endpoint.substr(index + 1),
      ];
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
  Deno.exit(1);
}

test[name](Deno.args.slice(1));
