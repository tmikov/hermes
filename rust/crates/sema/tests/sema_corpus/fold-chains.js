// Constant `+`/`-` chains: every link folds, so the whole chain collapses
// to a single literal (SemanticResolver.cpp:420-429).
1 + 2 - 3;
1 + 2 + 3 + 4 + 5;
10 - 4 - 3;
// The non-linearized path (cpp:432-435): a single fold attempt after the
// children are visited.
6 * 7;
100 / 8;
7 % 4;
1 << 31;
-8 >> 2;
-8 >>> 28;
5 & 3;
5 | 3;
5 ^ 3;
