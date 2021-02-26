// console.log(Deno.core.jsonOpSync("op_webcrypto_generate_key", { modulusLength: 4096/2, exponent: 101 }))
// console.log(crypto.generateKey({ name: "RsaPss",  modulusLength: 4096, publicModulus: 2 }, true, ["Sign"]))
console.log(crypto.generateKey({ name: "RsaPss",  modulusLength: 4096, publicModulus: 2 }, true, ["Sign"]))