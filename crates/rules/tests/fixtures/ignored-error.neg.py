def cleanup(name):
    try:
        unset_variable(name)
    except KeyError as e:
        raise CleanupError(name) from e
