var o = {
  m<T>(x: T): T {
    return x;
  },
  get x(): number {
    return 1;
  },
  set y(v: number): void {},
  get<T>(x: T): T {
    return x;
  },
  set<T>(x: T): T {
    return x;
  },
  async<T>(x: T): T {
    return x;
  },
};
