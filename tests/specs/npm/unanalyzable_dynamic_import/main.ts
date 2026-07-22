const specifier = "npm:@denotest/add";
const { add } = await import(specifier);

console.log(add(1, 2));
