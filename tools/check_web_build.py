"""What the published web build asks its host for, and whether the host provides it.

The wasm module's import section is a precise contract: every `env.<name>` in it
must be defined by one of the scripts the page loads, or the module fails to
instantiate and the game does not start. Nothing had ever checked it. §5.56 found
the toolkit's web saves silently sharing a localStorage key with four other
games, and the reason that survived is the same reason this could: nothing in
this project ever looked at what was actually deployed.

Run against the deploy root, not the source tree — the publish pipeline deletes
per-game copies of the shared runtime scripts and expects them at
`shared-assets/runtime/`, which is a real thing that can go wrong.
"""

import re
import sys
from pathlib import Path


def leb128(data, at):
    """Read an unsigned LEB128, returning (value, next offset)."""
    value = shift = 0
    while True:
        byte = data[at]
        at += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, at
        shift += 7


def imports(wasm):
    """Every (module, field) the module imports."""
    data = wasm.read_bytes()
    if data[:4] != b"\0asm":
        raise SystemExit(f"{wasm} is not a wasm module")
    at = 8
    found = []
    while at < len(data):
        section, at = leb128(data, at)
        size, at = leb128(data, at)
        end = at + size
        if section == 2:
            count, at = leb128(data, at)
            for _ in range(count):
                length, at = leb128(data, at)
                module = data[at:at + length].decode(); at += length
                length, at = leb128(data, at)
                field = data[at:at + length].decode(); at += length
                kind = data[at]; at += 1
                # Only function imports carry a type index; the rest carry a
                # descriptor this does not need.
                _, at = leb128(data, at) if kind == 0 else (0, at + 1)
                found.append((module, field))
        at = end
    return found


def env_object(text):
    """The keys of the literal `env:{...}` the runtime builds.

    Taken by matching braces rather than by regex, because the bundle ships
    minified: the whole object is one line, so anything anchored to a line start
    finds nothing. The first attempt at this reported 105 of 113 imports missing,
    which was the detector describing itself rather than the build.
    """
    names = set()
    for opening in (m.end() for m in re.finditer(r"env\s*:\s*\{", text)):
        depth, at = 1, opening
        while at < len(text) and depth:
            if text[at] == "{":
                depth += 1
            elif text[at] == "}":
                depth -= 1
            at += 1
        body = text[opening:at]
        # Three spellings, all of which the bundle uses: `name:function(...)`,
        # `name:(...)=>`, and ES6 shorthand `name,` where the function is
        # declared at the top level. Missing the third reported `dpi_scale` and
        # `init_webgl` as absent when both were plainly there.
        names |= set(re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(?:function|\()", body))
        names |= set(re.findall(r"[{,]\s*([A-Za-z_][A-Za-z0-9_]*)\s*[,}]", body))
    return names


def provided(scripts):
    """Names the loaded JS defines, however it spells the assignment."""
    names = set()
    for script in scripts:
        text = script.read_text(encoding="utf-8", errors="replace")
        # Plugins attach after the fact: `importObject.env.foo = function...`.
        names |= set(re.findall(r"env\.([A-Za-z_][A-Za-z0-9_]*)\s*=", text))
        names |= set(re.findall(r"""env\[["']([A-Za-z_][A-Za-z0-9_]*)["']\]""", text))
        names |= env_object(text)
    return names


def main(root):
    root = Path(root)
    wasms = list(root.glob("*.wasm"))
    if not wasms:
        raise SystemExit(f"no wasm in {root}")
    page = next(root.glob("index.html"))
    html = page.read_text(encoding="utf-8", errors="replace")

    scripts = []
    missing_files = []
    for src in re.findall(r'<script[^>]+src="([^"]+)"', html):
        path = (root / src).resolve()
        (scripts if path.is_file() else missing_files).append(path)
    if missing_files:
        for path in missing_files:
            print(f"the page loads a script that is not there: {path}")
        return 1

    have = provided(scripts)
    wanted = [field for module, field in imports(wasms[0]) if module == "env"]
    if not wanted:
        raise SystemExit("the wasm imports nothing from env; the parse is wrong")

    absent = sorted(name for name in wanted if name not in have)
    print(f"{wasms[0].name}: {len(wanted)} env imports, {len(scripts)} scripts, "
          f"{len(absent)} unsatisfied")
    for name in absent:
        print(f"  nothing defines env.{name}")
    return 1 if absent else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
