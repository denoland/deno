//a.ts
import("./b.mjs").catch(e=>{
  console.log("caught import b.mjs error")
  console.error(e)
})

import("./c.mjs").catch(e=>{
  console.log("caught import c.mjs error")
  console.error(e)
})