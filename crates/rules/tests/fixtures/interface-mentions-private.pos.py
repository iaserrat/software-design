class IndexLookup:
    def _is_ready(self):
        return self.ready

    def get_next(self):
        """Returns the next object. Calls _is_ready until enough hashes are buffered."""
        while not self._is_ready():
            self.poll()
        return self.take()
