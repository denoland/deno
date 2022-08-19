import { nextTick } from "https://deno.land/std@0.126.0/node/_next_tick.ts"

let total = parseInt(Deno.args[0], 10)
const count = parseInt(Deno.args[1], 10)

function bench (fun) {
  const start = Date.now()
  for (let i = 0; i < count; i++) fun()
  const elapsed = Date.now() - start
  const rate = Math.floor(count / (elapsed / 1000))
  console.log(`time ${elapsed} ms rate ${rate}`)
  if (--total) nextTick(() => bench(fun))
}

bench(() => performance.now())
