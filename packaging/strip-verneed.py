#!/usr/bin/env python3
"""Remove a stale, unreferenced version requirement from an ELF's .gnu.version_r.

`patchelf --clear-symbol-version` drops the *symbol's* version reference but
leaves the Verneed/Vernaux entry in place (verified on patchelf 0.14.3 and
0.18.0). The dynamic loader enforces those entries whether or not any symbol
still binds to them, so the library keeps refusing to load on a host with an
older glibc. This unlinks the now-unreferenced Vernaux entry from its chain and
decrements the parent Verneed count.

Refuses to touch an entry that any symbol still binds to, so it cannot silently
produce a library with unresolvable symbols.

    strip-verneed.py <elf> <soname> <version>     e.g. lib.so libm.so.6 GLIBC_2.35
"""
import struct
import sys

SHT_GNU_VERSYM = 0x6FFFFFFF
SHT_GNU_VERNEED = 0x6FFFFFFE


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    path, want_file, want_ver = sys.argv[1], sys.argv[2], sys.argv[3]

    with open(path, "rb") as fh:
        data = bytearray(fh.read())
    if data[:4] != b"\x7fELF" or data[4] != 2 or data[5] != 1:
        sys.exit(f"{path}: not a little-endian ELF64")

    (e_shoff,) = struct.unpack_from("<Q", data, 0x28)
    e_shentsize, e_shnum = struct.unpack_from("<HH", data, 0x3A)
    sections = []
    for i in range(e_shnum):
        off = e_shoff + i * e_shentsize
        sh_type, = struct.unpack_from("<I", data, off + 4)
        sh_offset, sh_size = struct.unpack_from("<QQ", data, off + 0x18)
        sh_link, = struct.unpack_from("<I", data, off + 0x28)
        sections.append((sh_type, sh_offset, sh_size, sh_link))

    verneed = next((s for s in sections if s[0] == SHT_GNU_VERNEED), None)
    if verneed is None:
        sys.exit(f"{path}: no .gnu.version_r section")
    _, vn_off, _, vn_link = verneed
    _, str_off, _, _ = sections[vn_link]

    # Version indices any symbol actually binds to.
    used = set()
    versym = next((s for s in sections if s[0] == SHT_GNU_VERSYM), None)
    if versym is not None:
        _, vs_off, vs_size, _ = versym
        for i in range(vs_size // 2):
            (v,) = struct.unpack_from("<H", data, vs_off + i * 2)
            used.add(v & 0x7FFF)

    def string(off):
        end = data.index(b"\0", str_off + off)
        return data[str_off + off : end].decode()

    vn = vn_off
    while True:
        _, vn_cnt, vn_file, vn_aux, vn_next = struct.unpack_from("<HHIII", data, vn)
        if string(vn_file) == want_file:
            prev = None
            aux = vn + vn_aux
            for _ in range(vn_cnt):
                _, _, vna_other, vna_name, vna_next = struct.unpack_from("<IHHII", data, aux)
                if string(vna_name) == want_ver:
                    if (vna_other & 0x7FFF) in used:
                        sys.exit(f"{path}: {want_ver} still bound by symbols; refusing")
                    if prev is None:
                        struct.pack_into("<I", data, vn + 8, vn_aux + vna_next if vna_next else 0)
                    else:
                        struct.pack_into("<I", data, prev + 12, aux + vna_next - prev if vna_next else 0)
                    struct.pack_into("<H", data, vn + 2, vn_cnt - 1)
                    with open(path, "wb") as fh:
                        fh.write(data)
                    print(f"{path}: unlinked {want_file} {want_ver}")
                    return 0
                if not vna_next:
                    break
                prev, aux = aux, aux + vna_next
        if not vn_next:
            break
        vn += vn_next
    sys.exit(f"{path}: no requirement on {want_file} {want_ver} found")


if __name__ == "__main__":
    sys.exit(main())
