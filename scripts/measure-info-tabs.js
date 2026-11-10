// Confirm (or refute) the constants behind scripts/check-info-tabs.py.
//
// That check estimates a tab strip's rendered width as `7 * cost - 11 * tabs`,
// where cost is heading characters plus five per tab. The 7px is derived from
// `.tab`'s 14px font size, NOT measured. This measures it.
//
// Paste into the console on any glpi-inv page, with an info modal OPEN so the
// real tab classes are in the DOM. Reports:
//   - px per character, against the 7 assumed
//   - the modal's content width at this viewport
//   - whether the modal or the strip is actually overflowing right now
(() => {
  const strip = document.querySelector('[role="tablist"].tabs-border');
  if (!strip) return 'open an info modal first - no tab strip in the DOM';

  const tabs = [...strip.querySelectorAll('[role="tab"]')];
  const chars = tabs.reduce((n, t) => n + t.textContent.trim().length, 0);
  const cost = chars + 5 * tabs.length;

  // Per-character advance, backing out each tab's horizontal padding.
  const pad = tabs.reduce((n, t) => {
    const s = getComputedStyle(t);
    return n + parseFloat(s.paddingLeft) + parseFloat(s.paddingRight);
  }, 0);
  const perChar = (strip.scrollWidth - pad) / chars;

  const modal = strip.closest('div[class*="max-h-"]');
  const ms = modal && getComputedStyle(modal);
  const content = modal
    ? modal.clientWidth - parseFloat(ms.paddingLeft) - parseFloat(ms.paddingRight)
    : null;

  return {
    tabs: tabs.length,
    chars,
    cost,
    stripScrollWidth: strip.scrollWidth,
    stripVisibleWidth: strip.clientWidth,
    stripOverflows: strip.scrollWidth > strip.clientWidth,
    pxPerChar: +perChar.toFixed(2),
    pxPerCharAssumed: 7,
    estimatedWidth: 7 * cost - 11 * tabs.length,
    modalContentWidth: content,
    modalOverflowsHorizontally: modal
      ? modal.scrollWidth > modal.clientWidth
      : null,
    pageOverflowsHorizontally:
      document.documentElement.scrollWidth > document.documentElement.clientWidth,
    // cost that would exactly fill this viewport's modal, at the measured rate
    budgetHere: content
      ? Math.floor((content + 11 * tabs.length) / perChar)
      : null,
  };
})();
