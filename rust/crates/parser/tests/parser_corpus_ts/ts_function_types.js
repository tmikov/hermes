type F = (x: number) => string;
type G = () => void;
type P = (number);
type R = (...args: number[]) => void;
type Ctor = new (x: A) => B;
type T2 = (this: X, y: Y) => Z;
type Gen = <T>(x: T) => T;
type CtorGen = new <T>(x: T) => T;
