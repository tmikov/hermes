interface I { x: number; f(): void }
interface J<T> extends A, B<T> { y: T }
