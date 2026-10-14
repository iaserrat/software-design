def cleanup(name):
    try:
        unset_variable(name)
    except KeyError:
        pass
