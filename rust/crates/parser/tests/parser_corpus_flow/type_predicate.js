type A = (x: mixed) => x is number;
type B = (x: mixed) => asserts x is T;
type C = (x: mixed) => asserts x;
type D = (x: mixed) => implies x is T;
type E = (x: mixed) => asserts;
type F = (x: mixed) => implies;
