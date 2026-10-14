export class BufferedStream implements Stream {
  read(count: number): Bytes { return this.inner.read(count); }
}
export class Child extends Base {
  constructor(name: string) { super(name); }
}
export function route(request: Request): Response {
  switch (request.kind) { case "file": return serveFile(request); default: return runScript(request); }
}
export function scaled(width: number): number { return scale(width, 2); }
export function copyBytes(offset: number, count: number): number { return copy(offset, count); }
export function respond(body: string): Response { return Response.create(body); }
