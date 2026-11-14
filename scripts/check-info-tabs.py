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

TWO FLOORS, BECAUSE A WIDTH BUDGET CANNOT SEE AN UNDERWEIGHT PANEL. The check
above only asks whether the strip fits. A panel with one short tab costs ~30
against a budget of 95 and passes forever, which is how seventeen of them
accumulated before anyone looked: every one was a click that revealed less
than a paragraph.

  MIN_TABS   One tab is not a tab strip. It offers no choice - the reader
             clicks to see the only thing there is - so the section belongs
             in the body, where it is already visible. Categorical, with no
             threshold to argue about.

  FLOOR      Tabs holding less than 400 characters between them are a choice
             between fragments. 400 is derived from the corpus median
             section, ~296 characters: a strip whose tabs together hold less
             than one and a half typical sections is not dividing anything.
             The thinnest panel that survives it holds 559, so this is a
             floor with room above it and not a line drawn under the status
             quo.

A section whose body is not a string literal - `&tile.cannot_check`, built
per tile - cannot be measured, so its panel is skipped by FLOOR rather than
guessed at. It is still subject to MIN_TABS and to the width budget.
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
    # inv.rs:741 - cost 117
    frozenset({'The columns', 'This page could not be built on the current API', 'Addresses are matched to the right interface'}),
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
MIN_TABS = 2
FLOOR = 400
ROOT = Path(__file__).resolve().parent.parent / "frontend" / "fro" / "src"


# Heading, then either a string-literal body or anything else (an expression,
# which cannot be measured). re.DOTALL is REQUIRED and is not decoration:
# `\\.` has to match a backslash followed by a newline, which is how every
# multi-line literal in this corpus is wrapped. Without it the match stops at
# the first line continuation and every long body measures as its first line -
# a mistake already made once while auditing this prose.
SECTION = re.compile(
    r'InfoSection::new\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*'
    r'(?:"((?:[^"\\]|\\.)*)"|([^,)]+))',
    re.DOTALL,
)


def unwrap(literal):
    """The rendered text of a Rust string literal, for length only."""
    # A line continuation and the indent that follows it render as nothing.
    text = re.sub(r"\\\n\s*", "", literal)
    text = re.sub(r"\\u\{([0-9a-fA-F]+)\}", lambda m: chr(int(m.group(1), 16)), text)
    return text.replace('\\"', '"').replace("\\'", "'").replace("\\\\", "\\")


def panels():
    """Every `sections: vec![...]` block, as (file, line, [(heading, body)]).

    `body` is None where the section's text is an expression rather than a
    literal, so a caller can tell "nothing there" from "not measurable".
    """
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
            sections = [
                (unwrap(m.group(1)), None if m.group(2) is None else unwrap(m.group(2)))
                for m in SECTION.finditer(block)
            ]
            if sections:
                line = text.count("\n", 0, match.start()) + 1
                yield path, line, sections


def main() -> int:
    over = []
    thin = []
    worst = 0
    thinnest = None
    exempt = 0
    for path, line, sections in panels():
        headings = [h for h, _ in sections]
        bodies = [b for _, b in sections]

        if len(sections) < MIN_TABS:
            thin.append((path, line, headings, None))
        elif all(b is not None for b in bodies):
            held = sum(len(b) for b in bodies)
            if held < FLOOR:
                thin.append((path, line, headings, held))
            else:
                thinnest = held if thinnest is None else min(thinnest, held)

        cost = sum(len(h) for h in headings) + PER_TAB * len(headings)
        if cost <= BUDGET:
            worst = max(worst, cost)
        elif frozenset(headings) in GRANDFATHERED:
            exempt += 1
        else:
            over.append((cost, path, line, headings))

    if not over and not thin:
        print(
            f"info tabs: ok - worst current panel {worst}, budget {BUDGET}"
            f", {exempt} grandfathered; thinnest holds {thinnest}, floor {FLOOR}"
        )
        return 0

    for cost, path, line, headings in sorted(over, reverse=True):
        rel = path.relative_to(ROOT.parent.parent.parent)
        print(f"{rel}:{line}: tab strip costs {cost}, budget {BUDGET}")
        for h in sorted(headings, key=len, reverse=True):
            print(f"    {len(h):3}  {h}")
        print("    shorten the headings - the explanation belongs in the body")

    for path, line, headings, held in thin:
        rel = path.relative_to(ROOT.parent.parent.parent)
        if held is None:
            print(f"{rel}:{line}: one tab is not a tab strip")
        else:
            print(f"{rel}:{line}: tabs hold {held} characters, floor {FLOOR}")
        for h in headings:
            print(f"        {h}")
        print("    fold the sections into the body - it already shows unclicked")

    return 1


if __name__ == "__main__":
    sys.exit(main())
