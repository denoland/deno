import j1, { $var } from "./subdir/json_1.json";
import j2 from "./subdir/json_2.json";
console.log($var);
console.log($var.a);
console.log(j1);
console.log(j1["with space"]);
console.log(j2);
