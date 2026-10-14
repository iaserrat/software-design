class BufferedStream(Stream):
    def read(self, count):
        return self.inner.read(count)

class Child(Base):
    def __init__(self, name):
        super().__init__(name)

def scaled(width):
    return scale(width, 2)
