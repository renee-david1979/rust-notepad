import { createTwoFilesPatch } from "diff";

/**
 * Build a git-style unified patch from two texts (pure; unit-tested).
 * Used by the diff view's "Copy as patch" / "Export .patch…" actions.
 */
export function buildUnifiedPatch(
  originalTitle: string,
  originalText: string,
  modifiedTitle: string,
  modifiedText: string,
): string {
  return createTwoFilesPatch(originalTitle, modifiedTitle, originalText, modifiedText);
}
