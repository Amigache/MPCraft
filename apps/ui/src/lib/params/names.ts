/**
 * The order the names of the parameters are shown in.
 *
 * The tree and the list agree on it: a parameter that is under a branch in the tree is
 * in the same place in the list.
 */

/** How two names are ordered. */
export function compareNames(left: string, right: string): number {
  if (left < right) {
    return -1;
  }

  return left > right ? 1 : 0;
}
