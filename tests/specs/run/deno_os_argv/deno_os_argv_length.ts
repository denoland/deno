// osArgv should end with the same args as Deno.args
const osArgv = Deno.osArgv;
const args = Deno.args;

// The tail of osArgv should match Deno.args
const tail = osArgv.slice(osArgv.length - args.length);
console.log(
  "tail matches args:",
  JSON.stringify(tail) === JSON.stringify(args),
);

// osArgv should be longer than Deno.args (has runtime + subcommand + script prefix)
console.log("osArgv longer:", osArgv.length > args.length);

// Deno.args count check
console.log("args count:", args.length);
