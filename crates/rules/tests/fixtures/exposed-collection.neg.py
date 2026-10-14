class HttpRequest:
    def __init__(self):
        self._params = {}

    def get_parameter(self, name):
        return self._params.get(name)
