class C<T> { x: number; private y: string; readonly z: T; static s: number; m(a: T): void {} opt?: boolean; }
class D<T> extends Base<T> { constructor() { super(); } }
const E = class<T> { v: T; };
