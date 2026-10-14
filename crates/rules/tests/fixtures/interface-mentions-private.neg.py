class IndexLookup:
    def _is_ready(self):
        return self.ready

    def get_next(self):
        """Returns the next object in the range, or None once the range is exhausted."""
        while not self._is_ready():
            self.poll()
        return self.take()
