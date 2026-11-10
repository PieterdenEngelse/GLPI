#!/usr/bin/env python3
"""Fail when an info panel's tab strip is wider than the modal that holds it.

`InfoButton`'s tab strip is `flex-nowrap overflow-x-auto`: too many tabs, or
headings written as sentences, and it scrolls sideways. The component keeps
its own scroller so the tabs stay reachable, but a strip that needs scrolling
is a panel the reader has to work to see all of, and the headings are the
cheap thing to change.

THE BUDGET IS DERIVED FROM THE MODAL'S GEOMETRY, not from the worst panel
that happens to exist. From the generated stylesheet: `.tab` is
`--tab-p: .75rem` (24px of padding per tab) at `font-size: .875rem`, and the
modal is `max-w-lg sm:max-w-2xl lg:max-w-3xl` with `p-5`, so its content box
is 472 / 632 / 728px. Taking the middle one - below `sm` the window is
genuinely narrow and scrolling is the acknowledged fallback - and ~7px per
character at 14px:

    width ~= 7 * cost - 11 * tabs      where cost = chars + 5 per tab

632px lands at cost ~95 for three to five tabs, which is the cap below. The
corpus median is 62, so this is a ceiling and not a target - see the note on
calibrating to the median rather than the cap.

The 7px figure is an ESTIMATE from the font size, not a measurement; the
docstring says so rather than pretending otherwise. To check it, paste
scripts/measure-info-tabs.js into the console on a page with an info panel.

GRANDFATHERED holds panels that predate this check and exceed the budget.
They are keyed on their exact set of headings, so editing any heading in one
drops it out of the exemption and it must then meet the budget. That is the
point: the debt is named rather than hidden behind a cap set high enough to
swallow it, and it shrinks as those panels are touched.
"""

import re
import sys
from pathlib import Path

# Panels that predate this check and exceed the budget. Keyed on the exact
# set of headings: edit any one of them and the exemption lapses, which is
# how this list is meant to shrink.
GRANDFATHERED = {
    # db.rs:387 - cost 144
    frozenset({'Why this is not visible anywhere else', 'Where the comparison comes from', 'Write-only fields are not counted', 'What it cannot tell you'}),
    # rights.rs:1347 - cost 139
    frozenset({'How it works', 'Why it matters on this page', 'Two kinds never get grouped', 'Not the same as the grant above', 'Not observed here'}),
    # db.rs:564 - cost 136
    frozenset({'Why the whole list is here', 'Why there is no Received column', 'Declared, readable, write-only', 'Not the same thing as db-read'}),
    # db.rs:760 - cost 135
    frozenset({'Where this list comes from', 'Two of them need 12.0.1', 'Case, and what empty means', 'All eighteen are reachable from this app'}),
    # inv.rs:118 - cost 119
    frozenset({'Why this is separate from Agents and Computers', 'Where these four read from', 'Refused is still the odd one out'}),
    # db.rs:48 - cost 117
    frozenset({'What is on it', 'Why only two tables', 'Not a map of GLPI', '\\u{201c}Schema\\u{201d} also means something else'}),
    # inv.rs:741 - cost 117
    frozenset({'The columns', 'This page could not be built on the current API', 'Addresses are matched to the right interface'}),
    # rights.rs:1192 - cost 117
    frozenset({'Why there is a database account at all', 'Why it is only two tables', 'What it costs', 'No password is stored'}),
    # inv.rs:872 - cost 115
    frozenset({'Why this is the failure worth catching', 'Where this comes from', "Does that step around GLPI's permissions?"}),
    # sec.rs:170 - cost 111
    frozenset({'Why a stuck task matters here', 'Why the headline figure is not shown', 'Unknown is most of this install'}),
    # db.rs:156 - cost 110
    frozenset({'Why the columns are listed at all', 'The name is plural, and that cost a GRANT', 'Where this comes from'}),
    # rights.rs:455 - cost 105
    frozenset({'The columns', 'Why the raw value is shown', 'The interface matters', 'Where to change any of this'}),
    # sec.rs:349 - cost 105
    frozenset({'Read the scope before the list', "Why this app's own sessions dominate", 'It starts at the upgrade'}),
}

BUDGET = 95
PER_TAB = 5
ROOT = Path(__file__).resolve().parent.parent / "frontend" / "fro" / "src"


def panels():
    """Every `sections: vec![...]` block, as (file, line, headings)."""
    for path in sorted(ROOT.rglob("*.rs")):
        text = path.read_text()
        for match in re.finditer(r"sections:\s*vec!\[", text):
            depth, i = 1, match.end()
            while depth and i < len(text):
                if text[i] == "[":
                    depth += 1
                elif text[i] == "]":
                    depth -= 1
                i += 1
            block = text[match.end() : i]
            headings = re.findall(
                r'InfoSection::new\(\s*"((?:[^"\\]|\\.)*)"', block
            )
            if headings:
                line = text.count("\n", 0, match.start()) + 1
                yield path, line, headings


def main() -> int:
    over = []
    worst = 0
    exempt = 0
    for path, line, headings in panels():
        cost = sum(len(h) for h in headings) + PER_TAB * len(headings)
        if cost <= BUDGET:
            worst = max(worst, cost)
            continue
        if frozenset(headings) in GRANDFATHERED:
            exempt += 1
            continue
        over.append((cost, path, line, headings))

    if not over:
        print(
            f"info tabs: ok - worst current panel {worst}, budget {BUDGET}"
            f", {exempt} grandfathered"
        )
        return 0

    for cost, path, line, headings in sorted(over, reverse=True):
        rel = path.relative_to(ROOT.parent.parent.parent)
        print(f"{rel}:{line}: tab strip costs {cost}, budget {BUDGET}")
        for h in sorted(headings, key=len, reverse=True):
            print(f"    {len(h):3}  {h}")
        print("    shorten the headings - the explanation belongs in the body")
    return 1


if __name__ == "__main__":
    sys.exit(main())
