#!/usr/bin/env python3
# Compressione del journal (solo misura): file intero a blocchi di 4 MB (come segmenti
# indipendenti) e per blocco della chain (record singoli), con zstd, lz4, gzip, xz.
import sys, time, zlib, lzma, struct
import os
import zstandard, lz4.frame
# REPEAT giri (default 3): velocità per giro, poi mediana; ogni giro verifica byte per byte.
REPEAT = int(os.environ.get("REPEAT", "3"))
path = sys.argv[1]
data = open(path, 'rb').read()[: int(sys.argv[2]) if len(sys.argv) > 2 else None]
MB = 1e6
def chunks(n):
    return [data[i:i+n] for i in range(0, len(data), n)]
def records():
    out, off = [], 0
    while off + 4 <= len(data):
        n = struct.unpack_from('<I', data, off)[0]
        if off + 4 + n > len(data): break
        out.append(data[off+4: off+4+n]); off += 4 + n
    return out
algos = {
  'zstd-1': (lambda b: zstandard.ZstdCompressor(level=1).compress(b), lambda b: zstandard.ZstdDecompressor().decompress(b)),
  'zstd-3': (lambda b: zstandard.ZstdCompressor(level=3).compress(b), lambda b: zstandard.ZstdDecompressor().decompress(b)),
  'zstd-9': (lambda b: zstandard.ZstdCompressor(level=9).compress(b), lambda b: zstandard.ZstdDecompressor().decompress(b)),
  'zstd-19': (lambda b: zstandard.ZstdCompressor(level=19).compress(b), lambda b: zstandard.ZstdDecompressor().decompress(b)),
  'lz4': (lambda b: lz4.frame.compress(b), lambda b: lz4.frame.decompress(b)),
  'gzip-6': (lambda b: zlib.compress(b, 6), lambda b: zlib.decompress(b)),
  'xz-6': (lambda b: lzma.compress(b, preset=6), lambda b: lzma.decompress(b)),
}
print(f"journal {path}: {len(data)/MB:.0f} MB")
print(f"| algoritmo | unità | rapporto | MB/s compressione, mediana ({REPEAT} giri) | MB/s decompressione, mediana ({REPEAT} giri) |")
print("|---|---|---|---|---|")
units = [("segmenti da 4 MB", chunks(4 << 20))]
if os.environ.get("ONLY_SEGMENTS") != "1":
    units.append(("singolo blocco", records()))
for unit, parts in units:
    for name, (c, d) in algos.items():
        if unit == "singolo blocco" and name in ("zstd-19", "xz-6"): continue
        cs, ds = [], []
        for _ in range(REPEAT):
            t = time.perf_counter(); out = [c(p) for p in parts]; cs.append(len(data)/MB/(time.perf_counter() - t))
            t = time.perf_counter(); back = [d(p) for p in out]; ds.append(len(data)/MB/(time.perf_counter() - t))
            assert back == parts
        size = sum(map(len, out))
        med = lambda v: sorted(v)[len(v)//2]
        fmt = lambda v: " / ".join(f"{x:.0f}" for x in v)
        print(f"| {name} | {unit} | {len(data)/size:.2f}x ({size/MB:.0f} MB) | {med(cs):.0f} ({fmt(cs)}) | {med(ds):.0f} ({fmt(ds)}) |", flush=True)
