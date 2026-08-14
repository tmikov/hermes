type A = (x: number, y: string) => void;
type B = () => void;
type C = (number) => string;
type D = (...rest: T) => void;
type E = <T>(x: T) => T;
type F = (this: X, a: Y) => Z;
type G = X => Y;
type H = (a?: T) => U;
