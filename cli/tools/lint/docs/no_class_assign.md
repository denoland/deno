Disallows modifying variables of class declarations

Declaring a class such as `class A {}`, creates a variable `A`. Like any
variable this can be modified or reassigned. In most cases this is a mistake and
not what was intended.

### Invalid:

```typescript
class A {}
A = 0; // reassigning the class variable itself
```

### Valid:

```typescript
class A {}
let c = new A();
c = 0; // reassigning the variable `c`
```
