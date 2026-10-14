function Badge({ label, tone }: { label: string; tone: string }) {
  return <span className={tone}>{label}</span>;
}
export function Inbox() { return <Badge tone="info" label="Inbox" />; }
export function Sent() { return <Badge label="Sent" tone="info" />; }
export function Drafts() { return <Badge tone="info" label="Drafts" />; }
