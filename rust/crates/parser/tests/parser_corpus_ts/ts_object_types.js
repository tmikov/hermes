type O = { a: number; b?: string; c: boolean };
type M = { f(x: number): void };
type Call = { (x: A): B };
type Idx = { [k: string]: number };
type Mixed = { a: number, f(): void, [k: string]: any };
type Computed = { ['x']: number };
