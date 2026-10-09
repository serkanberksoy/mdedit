#!/usr/bin/env python3
"""Release bookkeeping for one version (see VERSION.md, "On every bump").

    scripts/release.py X.Y.Z ENTRY.md

Sets the version in Cargo.toml, README.md and example_mds/feature_showcase.md,
adds ENTRY.md (the "### Added / Changed / Fixed" text) as the newest
VERSION.md entry, and updates the README's feature count. Then run
`UPDATE_SNAPSHOTS=1 cargo test --test features` (if statuses changed) and
`scripts/check.sh`.
"""
import datetime
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def sub(path, pattern, replacement):
    file = ROOT / path
    # A function, so the replacement's backslashes stay as written.
    text, n = re.subn(pattern, lambda _: replacement, file.read_text(), count=1, flags=re.M)
    if not n:
        sys.exit(f"{path}: no match for {pattern!r}")
    file.write_text(text)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    version, entry = sys.argv[1], Path(sys.argv[2]).read_text().strip()
    today = datetime.date.today().isoformat()
    sub('Cargo.toml', r'^version = ".*"', f'version = "{version}"')
    sub('VERSION.md', r'Current version: \*\*.*\*\*', f'Current version: **{version}**')
    sub('VERSION.md', r'^---\n\n## ', f'---\n\n## {version} ({today})\n\n{entry}\n\n## ')
    sub('README.md', r'\*\*Version:\*\* [0-9.]+', f'**Version:** {version}')
    sub('example_mds/feature_showcase.md', r'^version: .*', f'version: {version}')
    features = (ROOT / 'requirements/obsidian_features.md').read_text().splitlines()
    rows = [l for l in features if re.match(r'\| [A-Z]+-\d+ ', l)
            and '| Wrapper |' not in l and '| Dropped |' not in l]
    done = sum(1 for l in rows if '| ✅ |' in l or '| 🟡 |' in l)
    sub('README.md', r'\(\d+ of \d+ tracked', f'({done} of {len(rows)} tracked')
    print(f'{version}: {done} of {len(rows)} features')


if __name__ == '__main__':
    main()
