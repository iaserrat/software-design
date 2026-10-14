export class HttpRequest {
  private params: Map<string, string> = new Map();
  private version: string = "1.1";
  getParameter(name: string): string | undefined { return this.params.get(name); }
  getVersion(): string { return this.version; }
}
