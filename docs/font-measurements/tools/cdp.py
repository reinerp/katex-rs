# Minimal raw CDP client (websocket, no deps) with event capture. Used inside sbx --no-net with chrome on loopback.
import socket, os, base64, json, struct, time, urllib.request, sys
sys.path.insert(0, '/home/agents/code/agentserver/scripts')

class WS:
    def __init__(s, url):
        host, rest = url.split('://', 1)[1].split('/', 1); h, p = host.split(':')
        s.s = socket.create_connection((h, int(p))); s.s.settimeout(120); s.buf = b''; s.n = 0; s.events = []
        key = base64.b64encode(os.urandom(16)).decode()
        s.s.sendall(f"GET /{rest} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n".encode())
        while b'\r\n\r\n' not in s.buf: s.buf += s.s.recv(4096)
        _, _, s.buf = s.buf.partition(b'\r\n\r\n')
    def send(s, o):
        d = json.dumps(o).encode(); m = os.urandom(4); n = len(d); hdr = bytes([0x81])
        if n < 126: hdr += bytes([0x80 | n])
        elif n < 65536: hdr += bytes([0x80 | 126]) + struct.pack('>H', n)
        else: hdr += bytes([0x80 | 127]) + struct.pack('>Q', n)
        s.s.sendall(hdr + m + bytes(b ^ m[i % 4] for i, b in enumerate(d)))
    def need(s, n):
        while len(s.buf) < n:
            c = s.s.recv(1 << 20)
            if not c: raise EOFError
            s.buf += c
        r = s.buf[:n]; s.buf = s.buf[n:]; return r
    def recv(s):
        msg = b''
        while True:
            h = s.need(2); fin = h[0] & 0x80; n = h[1] & 0x7f
            if n == 126: n = struct.unpack('>H', s.need(2))[0]
            elif n == 127: n = struct.unpack('>Q', s.need(8))[0]
            msg += s.need(n)
            if fin: break
        return json.loads(msg)
    def call(s, method, **params):
        s.n += 1; me = s.n; s.send({'id': me, 'method': method, 'params': params})
        while True:
            r = s.recv()
            if r.get('id') == me: return r.get('result', r)
            if 'method' in r: s.events.append(r)
    def pump(s, until_method, timeout=60):
        end = time.time() + timeout
        while time.time() < end:
            for i, e in enumerate(s.events):
                if e['method'] == until_method: return s.events.pop(i)
            r = s.recv()
            if 'method' in r: s.events.append(r)
        raise TimeoutError(until_method)

def new_tab(port, url='about:blank'):
    t = json.loads(urllib.request.urlopen(urllib.request.Request(f'http://127.0.0.1:{port}/json/new?{url}', method='PUT')).read())
    return t
def close_tab(port, tid):
    try: urllib.request.urlopen(f'http://127.0.0.1:{port}/json/close/{tid}').read()
    except Exception: pass
