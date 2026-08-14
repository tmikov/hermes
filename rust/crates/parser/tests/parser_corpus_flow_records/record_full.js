record Point<T> implements I, J<K> {
  x: number, y: T,
  static origin: Point = mk(),
  dist(o: Point): number { return 0; }
  async *gen<U>(): U {}
}
