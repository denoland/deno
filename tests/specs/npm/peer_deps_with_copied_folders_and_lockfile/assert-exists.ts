if (Deno.args.length === 0) {
  console.error("no args");
  Deno.exit(1);
}

function exists(path: string) {
  try {
    Deno.statSync(path);
    return true;
  } catch (error) {
    console.error(error);
    return false;
  }
}

function expandBraces(pattern: string): string[] {
  if (!pattern.includes("{")) return [pattern];

  const results = [];
  const match = pattern.match(/\{([^{}]*)\}/);

  if (!match || !match.index) return [pattern];

  const prefix = pattern.slice(0, match.index);
  const suffix = pattern.slice(match.index + match[0].length);
  const options = match[1].split(",");

  for (const option of options) {
    const newPattern = prefix + option + suffix;
    const expanded = expandBraces(newPattern);
    results.push(...expanded);
  }

  return results;
}

const paths = Deno.args.map((arg) => arg.trim()).flatMap(expandBraces);

for (const path of paths) {
  if (!exists(path)) {
    console.error(`${path} does not exist`);
    Deno.exit(1);
  }
}
