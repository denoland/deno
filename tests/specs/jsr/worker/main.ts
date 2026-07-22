import { addAsync } from "jsr:@denotest/worker";

addAsync(1, 2).then((result) => {
  console.log(result);
});
