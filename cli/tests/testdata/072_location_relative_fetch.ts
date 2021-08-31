const response = await fetch("fetch/hello.txt");
console.log(await response.text());
