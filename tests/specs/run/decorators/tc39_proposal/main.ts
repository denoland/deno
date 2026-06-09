// deno-lint-ignore-file no-explicit-any
function logged(value: any, { kind, name }: { kind: string; name: string }) {
  if (kind === "method") {
    return function (...args: unknown[]) {
      console.log(`starting ${name} with arguments ${args.join(", ")}`);
      // @ts-ignore this has implicit any type
      const ret = value.call(this, ...args);
      console.log(`ending ${name}`);
      return ret;
    };
  }
}

const contexts: ClassMemberDecoratorContext[] = [];

function collect(_value: unknown, context: ClassMemberDecoratorContext) {
  contexts.push(context);
}

class C {
  @logged
  m(arg: number) {
    console.log("C.m", arg);
  }

  @collect
  method() {}

  @collect
  get getter() {
    return 1;
  }

  @collect
  set setter(_value: number) {}

  @collect
  accessor accessor = 1;

  @collect
  field = 1;

  @collect
  static staticMethod() {}
}

const instance = new C();
instance.m(1);

function assert(condition: unknown, message: string) {
  if (!condition) {
    throw new Error(message);
  }
}

assert(contexts.length === 6, "expected all member decorators to run");

for (const context of contexts) {
  assert("access" in context, `${context.kind} context is missing access`);
  assert(
    "has" in context.access,
    `${context.kind} context.access is missing has`,
  );

  const receiver = context.static ? C : instance;
  assert(
    context.access.has(receiver),
    `${context.kind} context.access.has did not match its receiver`,
  );
  assert(
    !context.access.has({}),
    `${context.kind} context.access.has matched an unrelated object`,
  );
}

console.log("decorator access.has works");
