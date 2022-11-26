let i = 0;
let j;
j = setInterval(() => {
    console.log("hello");
    i++;
    if (i > 3) {
        clearInterval(j);
    }
}, 1000);