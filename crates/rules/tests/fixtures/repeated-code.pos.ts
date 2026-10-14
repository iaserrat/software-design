export function onRead(packet: Packet): void {
  if (packet.length < READ_SIZE) {
    log.warn("short read packet", packet.id);
    drop(packet);
  }
}
export function onWrite(frame: Packet): void {
  if (frame.length < WRITE_SIZE) {
    log.warn("short write packet", frame.id);
    drop(frame);
  }
}
