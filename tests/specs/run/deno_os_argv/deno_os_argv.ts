const osArgv = Deno.osArgv;

// osArgv should contain the full OS argv
console.log("is array:", Array.isArray(osArgv));
console.log("frozen:", Object.isFrozen(osArgv));

// First element should be the runtime path/name
console.log("has runtime:", osArgv[0].length > 0);

// Should contain "run" subcommand
console.log("has subcommand:", osArgv.includes("run"));

// Should contain the script name
console.log(
  "has script:",
  osArgv.some((a: string) => a.endsWith("deno_os_argv.ts")),
);

// Should contain user args
console.log("has user args:", osArgv.includes("--user-arg1"));

// osArgv0 should be a non-empty string
console.log("osArgv0 is string:", typeof Deno.osArgv0 === "string");
console.log("osArgv0 non-empty:", Deno.osArgv0.length > 0);
