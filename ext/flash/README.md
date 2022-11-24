# flash

Flash is a fast HTTP/1.1 server implementation for Deno.

```js
serve({ fetch: (req) => new Response("Hello World") });
```

localset

 -> non thread safe function

 -> function for later

drop(localset)

op_state

-> lifetime of the isolate

-> Rust &'static

-> let local_task_set: &'asd TaskSet = state.borrow();
(&'static TaskSet)

-> unsafe { std::mem::transmute(local_task_set) }