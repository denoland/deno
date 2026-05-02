import * as pkg from "package";
import * as sub from "package/sub";

// should cause a type error where the type of value is "expected"
{
  const local: "not" = pkg.value;
  console.log(local);
}

// should also cause a type error
{
  const local: "not" = sub.value;
  console.log(local);
}
