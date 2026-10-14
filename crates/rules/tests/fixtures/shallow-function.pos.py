def _add_null_value(attrs, name):
    attrs[name] = None

def reset(attrs):
    _add_null_value(attrs, "id")
