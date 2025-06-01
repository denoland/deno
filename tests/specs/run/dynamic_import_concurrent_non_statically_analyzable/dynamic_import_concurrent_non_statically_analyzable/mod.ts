// sleep a bit so many concurrent tasks end up
// attempting to build the graph at the same time
import "http://localhost:4545/sleep/10";

export function outputValue() {
  console.log(parseInt(new URL(import.meta.url).hash.slice(1), 10));
}
