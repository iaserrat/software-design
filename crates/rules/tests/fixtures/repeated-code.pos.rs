fn on_read(packet: &Packet) {
    if packet.len() < READ_SIZE {
        warn!("short read packet {}", packet.id);
        drop_packet(packet);
    }
}
fn on_write(frame: &Packet) {
    if frame.len() < WRITE_SIZE {
        warn!("short write packet {}", frame.id);
        drop_packet(frame);
    }
}
