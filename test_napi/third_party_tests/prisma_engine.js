const prismaQueryEngine = Deno.core.dlopen(
  "node_modules/@prisma/engines/libquery_engine-darwin-arm64.dylib.node",
);

console.log(prismaQueryEngine.version());
const engine = new prismaQueryEngine.QueryEngine({
  datamodel: `
  generator client {
    provider = "prisma-client-js"
  }
  
  datasource db {
    provider = "sqlite"
    url      = "file:./dev.db"
  }
  `,
  env: {},
  logQueries: true,
  ignoreEnvVarErrors: false,
  logLevel: "debug",
  configDir: ".",
}, console.log);

await engine.connect({ enableRawQueries: true });
