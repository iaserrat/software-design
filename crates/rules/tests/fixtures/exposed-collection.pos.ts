export class HttpRequest {
  private params: Map<string, string> = new Map();
  getParams(): Map<string, string> { return this.params; }
}
