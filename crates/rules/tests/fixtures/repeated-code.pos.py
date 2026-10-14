def on_read(packet):
    if packet.length < READ_SIZE:
        log.warn("short read packet", packet.id)
        drop(packet)

def on_write(frame):
    if frame.length < WRITE_SIZE:
        log.warn("short write packet", frame.id)
        drop(frame)
