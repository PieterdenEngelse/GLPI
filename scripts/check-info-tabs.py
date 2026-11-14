#!/usr/bin/env python3
"""Fail when an info panel hides text behind a tab strip.

THE RULE: no sections. A tab strip that hides text the reader could already
see is a click that buys nothing, so the sections belong in the body where
they show unclicked.

This replaces a width budget plus two floors, which between them asked
whether a strip was too wide or too thin but never whether it should exist.
Measuring the corpus settled that: the largest panel held 2009 characters
across its tabs - about eighteen lines at the modal's width - so every one of
the 53 fitted a window unscrolled and none of them needed a strip. There was
no threshold to find, which is why there is no threshold here.

THE ONE EXEMPTION is a panel whose section bodies are not string literals -
sec.rs builds its sections per tile from `&tile.cannot_check`. Those cannot
be folded into a static body, so they keep their strip and are the only
thing the width budget below still governs.

THE WIDTH BUDGET, kept for that case. `InfoButton`'s strip is
`flex-nowrap overflow-x-auto`: too many tabs, or headings written as
sentences, and it scrolls sideways. From the generated stylesheet, `.tab` is
`--tab-p: .75rem` (24px of padding per tab) at `font-size: .875rem`, and the
modal is `max-w-lg sm:max-w-2xl lg:max-w-3xl` with `p-5`, so its content box
is 472 / 632 / 728px. Taking the middle one - below `sm` the window is
genuinely narrow and scrolling is the acknowledged fallback - and ~7px per
character at 14px:

    width ~= 7 * cost - 11 * tabs      where cost = chars + 5 per tab

632px lands at cost ~95 for three to five tabs, which is the cap below. The
7px figure is an ESTIMATE from the font size, not a measurement; to check it,
paste scripts/measure-info-tabs.js into the console on a page with an info
panel.

GRANDFATHERING IS GONE. It held eleven panels that predated the width budget;
all eleven lost their strips when the rule above was applied, so the list
emptied itself and the mechanism went with it.
"""

import re
import sys
from pathlib import Path

BUDGET = 95
PER_TAB = 5
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
    tabbed = []
    over = []
    runtime = 0
    worst = 0
    for path, line, sections in panels():
        headings = [h for h, _ in sections]
        bodies = [b for _, b in sections]

        # A section built at runtime cannot be folded into a static body.
        # That panel keeps its strip, and is the only one still measured.
        if any(b is None for b in bodies):
            runtime += 1
            cost = sum(len(h) for h in headings) + PER_TAB * len(headings)
            if cost <= BUDGET:
                worst = max(worst, cost)
            else:
                over.append((cost, path, line, headings))
            continue

        tabbed.append((path, line, headings))

    if not tabbed and not over:
        print(
            f"info tabs: ok - no tabbed panels except {runtime} built at "
            f"runtime, worst {worst} against budget {BUDGET}"
        )
        return 0

    for path, line, headings in tabbed:
        rel = path.relative_to(ROOT.parent.parent.parent)
        held = "sections"
        print(f"{rel}:{line}: {len(headings)} {held} behind a tab strip")
        for h in headings:
            print(f"        {h}")
        print("    fold them into the body - it already shows unclicked")

    for cost, path, line, headings in sorted(over, reverse=True):
        rel = path.relative_to(ROOT.parent.parent.parent)
        print(f"{rel}:{line}: tab strip costs {cost}, budget {BUDGET}")
        for h in sorted(headings, key=len, reverse=True):
            print(f"    {len(h):3}  {h}")
        print("    shorten the headings - the explanation belongs in the body")

    return 1


if __name__ == "__main__":
    sys.exit(main())
