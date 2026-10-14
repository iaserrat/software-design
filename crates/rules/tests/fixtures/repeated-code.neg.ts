export function onRead(packet: Packet): void {
  if (packet.length < READ_SIZE) drop(packet);
}
export function onWrite(frame: Packet): void {
  if (frame.length < WRITE_SIZE) drop(frame);
}
