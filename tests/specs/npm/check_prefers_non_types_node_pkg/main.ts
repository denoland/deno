// this lz-string@1.5 pkg has types only in the regular package and not the @types/lz-string pkg
import { compressToEncodedURIComponent } from "lz-string";

// cause a deliberate type checking error
console.log(compressToEncodedURIComponent(123));
