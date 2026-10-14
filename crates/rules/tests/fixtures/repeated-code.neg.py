def on_read(packet):
    if packet.length < READ_SIZE: drop(packet)

def on_write(frame):
    if frame.length < WRITE_SIZE: drop(frame)
