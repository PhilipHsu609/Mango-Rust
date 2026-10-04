import 'vitest';

declare module 'vitest' {
  export interface ProvidedContext {
    baseUrl: string;
    libraryPath: string;
  }
}
