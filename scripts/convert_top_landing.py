#!/usr/bin/env python3
"""One-off converter used for the Level 34 (Top Landing) literal -> level! DSL
conversion.

scripts/convert_level_dsl.py is the maintained, general room converter
(--scan / <level> / --write); this script is the scratch version that was
actually run for room 34, kept for the record. The two were compared: both
emit a byte-identical level! block for room 34 from the same HEAD literal.

Like the other scripts here it is never invoked by CI, build.rs, or the test
suite; re-running it after room 34 is converted fails its "block not found"
assertion by design. Verification it performed when run: every field of the
old literal was cross-checked against reference/c-original's Top Landing
entry, and (separately) the emitted block was re-parsed and diffed against
the literal it replaced.
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
rs = (ROOT / "src" / "levels.rs").read_text()
c_src = (ROOT / "reference" / "c-original" / "src" / "levels.c").read_text()

# ---- parse the Rust block for level 34 ----
rm = re.search(r"// Level 34: Top Landing\n    LevelData \{(.*?)\n    \},\n", rs, re.S)
assert rm, "rust level 34 block not found (already converted?)"
block = rm.group(1)
title = re.search(r'title: "([^"]+)"', block).group(1)
map_ = [int(x) for x in re.search(r"map: \[([^\]]+)\]", block).group(1).split(",")]
data = [int(x) for x in re.search(r"data: \[([^\]]+)\]", block, re.S).group(1).replace("\n", " ").split(",") if x.strip()]
assert len(data) == 512, f"data len {len(data)}"
gfx_raw = re.search(r"gfx: \[(.*?)\n        \],", block, re.S).group(1)
gfx = []
for tok in re.findall(r"GFX_SPACE|\[[^\]]+\]", gfx_raw):
    gfx.append(None if tok == "GFX_SPACE" else [int(x) for x in tok[1:-1].split(",")])
assert len(gfx) == 10, f"gfx slots {len(gfx)}"
info = re.findall(r"attr: (0x[0-9a-fA-F]+),\s*tile_type: TileType::(\w+)", block)
assert len(info) == 10, f"info slots {len(info)}"
info = [(int(a, 16), t) for a, t in info]
item_count = int(re.search(r"item_count: (\d+)", block).group(1))
item = [int(x) for x in re.search(r"item: \[([^\]]+)\]", block).group(1).split(",")]
assert len(item) == 12

# ---- parse the C entry (closers anchored at 8-space indent) ----
m = re.search(r'\.title = "Top Landing",(.*?)\n    \},', c_src, re.S)
assert m, "C Top Landing not found"
cblk = m.group(1)
c_map = [int(x) for x in re.search(r"\.map = \{([^}]+)\}", cblk).group(1).split(",")]
c_data = [int(x) for x in re.search(r"\.data = \{(.*?)\n        \},", cblk, re.S).group(1).replace("\n", " ").split(",") if x.strip()]
c_gfx_blk = re.search(r"\.gfx = \{(.*?)\n        \},", cblk, re.S).group(1)
c_gfx = []
for tok in re.findall(r"SPACE|\{[^{}]+\}", c_gfx_blk):
    c_gfx.append([0] * 8 if tok == "SPACE" else [int(x) for x in tok[1:-1].split(",")])
c_gfx += [[0] * 8] * (10 - len(c_gfx))  # rest zero-init in C
c_info_blk = re.search(r"\.info = \{(.*?)\n        \},", cblk, re.S).group(1)
c_info = re.findall(r"\{(0x[0-9a-fA-F]+),\s*(T_\w+)\}", c_info_blk)
c_info += [("0x00", "T_SPACE")] * (10 - len(c_info))  # T_SPACE == 0
c_itemcount = int(re.search(r"\.itemCount = (\d+)", cblk).group(1))
c_item = [int(x) for x in re.search(r"\.item = \{([^}]*)\}", cblk).group(1).split(",") if x.strip()]

# ---- cross-check Rust literal vs C original ----
assert title == "Top Landing"
assert map_ == c_map, (map_, c_map)
assert data == c_data, "data mismatch vs C"
assert len(c_data) == 512
assert item_count == c_itemcount, (item_count, c_itemcount)
assert item[:item_count] == c_item, (item, c_item)
assert all(v == 0 for v in item[item_count:])
for i in range(10):
    rust_gfx = gfx[i] or [0] * 8
    assert rust_gfx == c_gfx[i], (i, gfx[i], c_gfx[i])
    c_attr, c_type = c_info[i]
    assert info[i][0] == int(c_attr, 16), (i, info[i], c_attr)
    assert info[i][1].upper() == c_type[2:].upper(), (i, info[i][1], c_type)
print("cross-check vs C original: all fields match")

# ---- emit the level! block ----
def glyph(v):
    return "." if v == 0 else str(v)

rows = ["".join(glyph(v) for v in data[r * 32:(r + 1) * 32]) for r in range(16)]
assert all(len(r) == 32 for r in rows)
w = max(len(t) for _, t in info)
tiles = "\n".join(
    f"            {i}: {info[i][1]:<{w}} attr 0x{info[i][0]:02x}  gfx "
    + ("*;" if gfx[i] is None else "[" + ", ".join(str(v) for v in gfx[i]) + "];")
    for i in range(10)
)

def ex(v):
    return "none" if v == 0 else str(v)

items = ", ".join(str(v) for v in item[:item_count])
out = f"""    level! {{
        title: "{title}",
        exits {{ above: {ex(map_[0])}, right: {ex(map_[1])}, below: {ex(map_[2])}, left: {ex(map_[3])} }},
        tiles {{
{tiles}
        }},
        grid {{
{chr(10).join('            "%s",' % r for r in rows)}
        }},
        items [{items}],
    }},"""
print(out)

# ---- swap it into levels.rs ----
new_rs = rs[:rm.start()] + "// Level 34: Top Landing\n" + out + "\n" + rs[rm.end():]
(ROOT / "src" / "levels.rs").write_text(new_rs)
print("levels.rs rewritten")
