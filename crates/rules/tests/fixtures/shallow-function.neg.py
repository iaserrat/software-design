class Child(Base):
    def render(self):
        return draw(self)

def show(child):
    child.render()
