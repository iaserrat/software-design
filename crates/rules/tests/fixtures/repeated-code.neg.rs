fn on_read(packet: &Packet) { if packet.len() < READ_SIZE { drop_packet(packet) } }
fn on_write(frame: &Packet) { if frame.len() < WRITE_SIZE { drop_packet(frame) } }
