// this lz-string@1.3 pkg doesn't have types, but the @types/lz-string@1.3 does
import { compressToEncodedURIComponent } from "lz-string";

// cause a deliberate type checking error
console.log(compressToEncodedURIComponent(123));
