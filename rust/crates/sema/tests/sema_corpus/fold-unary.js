// astFoldUnaryExpression (cpp:499): `-`, `+` and `~` on a numeric literal.
-5;
+3.5;
~7;
- -5;
// Not folded: the operand is not a NumericLiteral.
-x;
// Not folded: the operator is not one of the three.
!true;
typeof 1;
void 0;
