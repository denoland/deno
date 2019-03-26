const worker = new Worker("tests/subdir/test_worker.js");

worker.postMessage("Hello World");
