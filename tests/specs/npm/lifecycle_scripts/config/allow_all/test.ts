const kind = Deno.args[0];

switch (kind) {
  case "all": {
    modify(true);
    install({
      expected: [
        "@denotest/node-addon@1.0.0",
        "@denotest/node-lifecycle-scripts@1.0.0",
      ],
    });
    break;
  }
  case "some": {
    modify(["npm:@denotest/node-addon@1.0.0"]);
    install({
      expected: ["@denotest/node-addon@1.0.0"],
    });
    break;
  }
  case "deny": {
    modify({
      allow: true,
      deny: ["npm:@denotest/node-addon@1.0.0"],
    });
    install({
      expected: ["@denotest/node-lifecycle-scripts@1.0.0"],
    });
    break;
  }
  case "arg": {
    modify({
      allow: true,
      deny: ["npm:@denotest/node-addon@1.0.0"],
    });
    install({
      additionalArgs: ["--allow-scripts"],
      expected: [
        "@denotest/node-addon@1.0.0",
        "@denotest/node-lifecycle-scripts@1.0.0",
      ],
    });
    break;
  }
  default:
    throw new Error("Unknown: " + kind);
}

function modify(value: any) {
  const obj = JSON.parse(Deno.readTextFileSync("deno.json"));
  obj.allowScripts = value;
  Deno.writeTextFileSync("deno.json", JSON.stringify(obj));
}

function install(options: {
  expected: (
    | "@denotest/node-addon@1.0.0"
    | "@denotest/node-lifecycle-scripts@1.0.0"
  )[];
  additionalArgs?: string[];
}) {
  const args = ["install", ...(options.additionalArgs ?? [])];
  console.error("args", args);
  const command = new Deno.Command(Deno.execPath(), {
    args,
    stderr: "piped",
    stdout: "piped",
  });
  const { stdout, stderr } = command.outputSync();
  const output = new TextDecoder().decode(stdout) +
    new TextDecoder().decode(stderr);

  for (const packageId of options.expected) {
    const expectedText = initializeText(packageId);
    if (!output.includes(expectedText)) {
      console.log(output);
      throw new Error("Could not find: " + expectedText);
    }
  }

  for (
    const packageId of [
      "@denotest/node-addon@1.0.0",
      "@denotest/node-lifecycle-scripts@1.0.0",
    ]
  ) {
    if (!options.expected.includes(packageId)) {
      const expectedText = initializeText(packageId);
      if (output.includes(expectedText)) {
        console.log(output);
        throw new Error("Output contained: " + packageId);
      }
    }
  }
}

function initializeText(packageId: string) {
  return `Initialize ${packageId}: running 'install' script`;
}
