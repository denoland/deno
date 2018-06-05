import helloWorld from "./subdir/hello_world.json";

function logHelloWorld(item: typeof helloWorld) {
    console.log(helloWorld);
}

logHelloWorld(helloWorld);
