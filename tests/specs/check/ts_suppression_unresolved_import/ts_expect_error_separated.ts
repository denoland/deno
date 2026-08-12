// @ts-expect-error
// an intervening comment and a blank line below are skipped, matching tsc

import value from "missing-package";

console.log(value);
