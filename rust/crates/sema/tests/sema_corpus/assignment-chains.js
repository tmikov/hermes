// `linearizeRight` + per-link validateAssignmentTarget (cpp:441-455).
var a, b, c;
a = b = c;
a = b = c = 1 + 2;
// The non-`=` path (cpp:457-461).
a += 1;
a.p = b;
// Destructuring targets recurse through validateAssignmentTarget
// (cpp:2709-2741).
[a, b] = [1, 2];
[a, , b] = [1, 2];
[a, ...b] = c;
({p: a, q: b} = c);
({p: a = 1} = c);
