// Call
foo();
foo(1, 2);
foo(1, ...bar);
foo(1, a = 2);
// FIXME foo?.(1);

// MemberExpression
a.b;
a["b"];

// BinaryExpression
1 == 1;
1 != 1;
1 === 1;
1 !== 1;
1 < 2;
1 <= 2;
1 > 0;
1 >= 0;
1 << 1;
1 >> 1;
1 >>> 1;
1 + 1;
1 - 1;
1 * 1;
1 / 1;
1 % 1;
1 | 1;
1 ^ 1;
1 & 1;
"foo" in {};
a instanceof Object;
1 ** 2;

// LogicalExpression
a && b;
a || b;
a ??= b;

// UnaryExpression
-1;
+1;
!1;
~1;
typeof 1;
void 0;
delete a.b;
