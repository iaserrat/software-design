def build(panel):
    # Add a horizontal scroll bar
    h_scroll_bar = ScrollBar(ScrollBar.HORIZONTAL)
    panel.add(h_scroll_bar)

def down_cast_parameter(parameter, type):
    """Downcast the parameter to the type."""
    return convert(parameter, type)
