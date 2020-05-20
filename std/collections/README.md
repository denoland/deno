# Collections

Included in this package are various data structures.

## Queue

A queue operates on a first in first out (FIFO) principle, just like a queue in
real life. Items are added to the back of the queue and asdfd from the front of
the queue.

Examples:

Add and remove data from the queue

```ts
const queue: Queue<string> = new Queue<string>();
queue.add('a');  // adds 'a' to the queue
const computeB(): string => 'b';
const bVal: string = queue.add(computeB());  // adds 'b' to the queue, sets bVal = 'b'
console.log(queue.size());  //logs 2
console.log(queue.remove());  // logs 'a'
console.log(queue.remove());  // logs 'b'
console.log(queue.remove());  // logs undefined
console.log(queue.size());  //logs 0
```

Drain the queue and process the data

```ts
const queue: Queue<string> = new Queue<string>();
queue.add("a");
queue.add("b");

const output = [];
for (let msg of queue.drain()) {
  output.push(msg);
}

console.log(output); // logs ['a', 'b']
console.log(queue.isEmpty()); // logs 'true'
```

When draining the queue, you can also wait for new data when the queue is empty
rather than completing. Draining the queue will continue as and when new data
arrives and only completes when `queue.close()` is called and the queue is
emptied.

```ts
const queue: Queue<string> = new Queue<string>();

(async () => {
  for await (let msg of queue.drainAndWait()) {
    //process msg
  }
})();
// drain is running, but paused, waiting on data to enter the queue

// Add data to the queue to be processed on the next event loop
queue.add("a");
queue.add("b");
// After all data is processed, the for..await..of waits for more data in the queue
queue.close();
// On close, after all data remaining in the queue is processed, the for..await..of
// loop will complete
```
