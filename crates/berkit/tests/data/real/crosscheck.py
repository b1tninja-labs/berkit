"""Cross-check the reference walker's top-level fields against the `cryptography` package (optional oracle).

    python crosscheck.py DIR

For each DIR/*.der compare version, serial number, the signature algorithm OID, the public key algorithm OID and
the validity instants, read by refdump.py's TLV walk, with what `cryptography` reports. Needs `cryptography`;
refdump.py itself does not.
"""

import datetime
import glob
import os
import sys
import warnings

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import refdump as R
from cryptography import x509

warnings.simplefilter("ignore")


def oid(content):
    a = [content[0] // 40, content[0] % 40]
    v = 0
    for b in content[1:]:
        v = (v << 7) | (b & 0x7F)
        if not b & 0x80:
            a.append(v)
            v = 0
    return ".".join(map(str, a))


def when(n):
    s = n.content.decode()
    if n.number == 23:
        yy = int(s[:2])
        s = ("20" if yy < 50 else "19") + s
    return datetime.datetime.strptime(s[:14], "%Y%m%d%H%M%S").replace(tzinfo=datetime.UTC)


bad = 0
files = sorted(glob.glob(os.path.join(sys.argv[1], "*.der")))
for f in files:
    der = open(f, "rb").read()
    c = x509.load_der_x509_certificate(der)
    t = R.tree(der).kids[0].kids
    hv = t[0].cls == 2
    i = 1 if hv else 0
    ver = t[0].kids[0].content[0] if hv else 0
    serial = int.from_bytes(t[i].content, "big", signed=True)
    mine = (ver, serial, oid(t[i + 1].kids[0].content), oid(t[i + 5].kids[0].kids[0].content),
            when(t[i + 3].kids[0]), when(t[i + 3].kids[1]))
    theirs = (c.version.value, c.serial_number, c.signature_algorithm_oid.dotted_string,
              c.public_key_algorithm_oid.dotted_string, c.not_valid_before_utc, c.not_valid_after_utc)
    if mine != theirs:
        bad += 1
        print("MISMATCH", f, mine, theirs)
print(len(files), "certificates,", bad, "mismatches")
sys.exit(bad != 0)
