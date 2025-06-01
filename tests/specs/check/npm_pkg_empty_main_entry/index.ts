// this package has an empty "main" entry in its package.json for both the package and @types/package
import { add } from "package";

const result: string = add(1, 2);
console.log(result);
