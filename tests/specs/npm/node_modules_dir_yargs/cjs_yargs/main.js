import yargs from "npm:yargs@15.4.1";

const args = yargs(["serve", "8000"])
  .command("serve [port]", "start the server", (yargs) => {
    return yargs
      .positional("port", {
        describe: "port to bind on",
        default: 5000,
      });
  }, (argv) => {
    console.info(`start server on :${argv.port}`);
  })
  .option("verbose", {
    alias: "v",
    type: "boolean",
    description: "Run with verbose logging",
  })
  .argv;

console.log(args);
