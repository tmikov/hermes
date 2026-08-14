class C<T> extends B<T> implements I, J<T> {
  x: number;
  +ro: T;
  -wo: U;
  readonly r: V;
  writeonly w: V;
  static s: number = 1;
  #p: T;
  static: number;
  m<U>(a: U): U {
    return a;
  }
  get g(): T {
    return this.x;
  }
  set t(v: T): void {}
}
var D = class <T> implements K {
  y: T;
};
class E implements L {}
class SW {
  static<W>() {}
}
