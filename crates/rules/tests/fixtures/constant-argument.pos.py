def respond(body, version):
    return Response(body, version)

def ok():
    return respond("ok", "HTTP/1.1")

def missing():
    return respond("missing", "HTTP/1.1")

def denied():
    return respond("denied", "HTTP/1.1")
