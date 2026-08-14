// A chain whose tail is non-constant folds its constant PREFIX only.
1 + 2 + x;
// ...while a chain whose HEAD is non-constant folds nothing at all: folding
// is strictly left-to-right and bottom-up, so the failure at `x + 1` stops
// the loop before `+ 2` is ever attempted (cpp:427-429).
x + 1 + 2;
// Same rule with a member expression as the non-constant head.
o.p - 1 - 2;
// Parenthesization changes the linearization, so this one DOES fold both.
x + (1 + 2);
