const response = await fetch("run/fetch/hello.txt");
console.log(await response.text());
