/* Stylesheet helpers for the contracts. A helper module, not a contract. */

/** The rules inside every `@media (prefers-reduced-motion: reduce)` block,
 * joined, so an assertion cannot be satisfied by a query elsewhere in the
 * file followed by an unrelated rule. */
export function reducedMotionCss(css) {
  const opener = /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{/g;
  const blocks = [];
  for (let match = opener.exec(css); match; match = opener.exec(css)) {
    let depth = 1;
    let at = opener.lastIndex;
    for (; at < css.length && depth > 0; at += 1) {
      if (css[at] === "{") depth += 1;
      else if (css[at] === "}") depth -= 1;
    }
    blocks.push(css.slice(opener.lastIndex, at - 1));
  }
  return blocks.join("\n");
}
