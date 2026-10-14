function Badge({ label, tone }: { label: string; tone: string }) {
  return <span className={tone}>{label}</span>;
}
export function Inbox() { return <Badge tone="info" label="Inbox" />; }
export function Sent() { return <Badge tone="info" label="Sent" />; }
export function Errors() { return <Badge tone="danger" label="Errors" />; }
export function Rows() { return <div><span className="row" /><span className="row" /><span className="row" /></div>; }
