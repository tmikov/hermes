type U = A | B | C;
type I = A & B & C;
type Mix = A & B | C & D;
type Arr = number[];
type Arr2 = number[][];
type Idx = Foo['bar'];
type Idx2 = Foo[number];
type ArrIdx = Foo[]['x'];
