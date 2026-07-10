import { Worker } from "node:worker_threads";

const OVERFLOW_VALUE = 2 ** 44;

for (
  const field of [
    "maxYoungGenerationSizeMb",
    "maxOldGenerationSizeMb",
    "codeRangeSizeMb",
    "stackSizeMb",
  ]
) {
  try {
    new Worker("setTimeout(() => {}, 1_000_000)", {
      eval: true,
      resourceLimits: {
        [field]: OVERFLOW_VALUE,
      },
    });
    console.log(`${field}: created`);
  } catch (err) {
    console.log(`${field}: ${err.name}: ${err.message}`);
  }
}
