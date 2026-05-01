import struct, re, os

def compile_po_to_mo(po_path, mo_path):
    with open(po_path, 'r', encoding='utf-8') as f:
        content = f.read()

    entries = []
    for match in re.finditer(
        r'msgid "(.+?)"\s*\nmsgstr "(.*?)"',
        content, re.DOTALL
    ):
        msgid = match.group(1)
        msgstr = match.group(2)
        if msgid:
            entries.append((msgid, msgstr))

    if not entries:
        print(f'No entries in {po_path}')
        return

    N = len(entries)
    header_sz = 28
    table_sz = N * 8 * 2

    orig_off = header_sz + table_sz
    orig_data = b''
    orig_offsets = []
    for eid, estr in entries:
        orig_offsets.append(len(orig_data))
        orig_data += eid.encode('utf-8') + b'\x00'

    trans_off = orig_off + len(orig_data)
    trans_data = b''
    trans_offsets = []
    for eid, estr in entries:
        trans_offsets.append(len(trans_data))
        trans_data += estr.encode('utf-8') + b'\x00'

    buf = bytearray()
    buf += struct.pack('<I', 0x950412de)
    buf += struct.pack('<I', 0)
    buf += struct.pack('<I', N)
    buf += struct.pack('<I', orig_off)
    buf += struct.pack('<I', trans_off)
    buf += struct.pack('<I', 0)
    buf += struct.pack('<I', 0)

    for i in range(N):
        buf += struct.pack('<II', len(entries[i][0].encode('utf-8')), orig_offsets[i])
    for i in range(N):
        buf += struct.pack('<II', len(entries[i][1].encode('utf-8')), trans_offsets[i])

    buf += orig_data
    buf += trans_data

    os.makedirs(os.path.dirname(mo_path), exist_ok=True)
    with open(mo_path, 'wb') as f:
        f.write(buf)
    print(f'{po_path} -> {mo_path}  ({N} strings)')

base = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(base)

compile_po_to_mo(os.path.join(root, 'po', 'zh_CN.po'), os.path.join(root, 'locale', 'zh_CN', 'LC_MESSAGES', 'zh_CN.mo'))
compile_po_to_mo(os.path.join(root, 'po', 'en_US.po'), os.path.join(root, 'locale', 'en_US', 'LC_MESSAGES', 'en_US.mo'))
compile_po_to_mo(os.path.join(root, 'po', 'zh_CN.po'), os.path.join(root, 'build', 'locale', 'zh_CN', 'LC_MESSAGES', 'zh_CN.mo'))
compile_po_to_mo(os.path.join(root, 'po', 'en_US.po'), os.path.join(root, 'build', 'locale', 'en_US', 'LC_MESSAGES', 'en_US.mo'))
