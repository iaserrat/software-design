# Blank space left on both sides of each line of text, in pixels.
TEXT_HORIZONTAL_PADDING = 4

def copy_bytes(offset, count):
    """Copies at most `count` bytes; returns how many were copied, 0 past the end."""
    return copy(offset, count)
