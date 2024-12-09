Disallows assignment to native Javascript objects

In Javascript, `String` and `Object` for example are native objects. Like any
object, they can be reassigned, but it is almost never wise to do so as this can
lead to unexpected results and difficult to track down bugs.

### Invalid:

```typescript
Object = null;
undefined = true;
window = {};
```
