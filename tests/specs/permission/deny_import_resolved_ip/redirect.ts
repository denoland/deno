// Regression test: a redirect target is subject to the same resolved-IP import
// deny check as the entry specifier. `localhost:4546` redirects to
// `localhost:4545`, so denying only the redirect target's address+port must
// block the load even though the specifier the user wrote points elsewhere.
import { add } from "http://localhost:4546/add.ts";

console.log(add(1, 2));
