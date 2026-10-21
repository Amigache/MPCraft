/**
 * The tree of the parameters.
 *
 * Mission Planner's parameter window is a tree on the left and the grid on the right: the
 * tree groups the parameters by their common prefix, and it is how a parameter of a
 * vehicle with hundreds of them is found.
 *
 * The tree is a pure function of the names. It is built here, next to the store, and not
 * in the component that draws it, so that it can be tested without a screen.
 *
 * The rule is upstream's: a parameter belongs to the branch of the longest prefix it
 * shares with its neighbours, and the prefix is split on the underscore. `ARMING_CHECK`
 * and `ARMING_OPTIONS` sit under `ARMING`; `RTL_ALT` and `RTL_SPEED` under `RTL`.
 *
 * A group of one is not a branch: a parameter that has no neighbour under its prefix —
 * `WP_RADIUS`, on its own — stays where it is, at the top, as upstream leaves it.
 *
 * A parameter whose name is the prefix of another one — `RTL` and `RTL_ALT` — is the
 * exception that makes the tree nest: the branch is drawn, and the parameter itself is
 * the branch, so that the branch holds every parameter that belongs to it.
 */

import { compareNames } from "./names";

/** The name of the node at the top of the tree. */
export const ROOT_LABEL = "All";

/** A node of the tree. */
export interface ParamTreeNode {
  /** The name shown on the node. */
  readonly label: string;
  /**
   * The prefix the node stands for, without the trailing underscore.
   *
   * The root's prefix is the empty string, which stands for every parameter.
   */
  readonly prefix: string;
  /** The parameter the node names, when a parameter has exactly this name. */
  readonly parameter: string | null;
  /** The children of the node. */
  readonly children: readonly ParamTreeNode[];
}

/**
 * Groups the names that sit under one prefix.
 *
 * The names are already known to start with the prefix. The one that *is* the prefix is
 * the parameter of the node itself and is not a child of it.
 */
function groupNames(
  names: readonly string[],
  path: readonly string[],
): readonly ParamTreeNode[] {
  const prefix = path.join("_");
  const buckets = new Map<string, string[]>();

  for (const name of names) {
    if (name === prefix) {
      continue;
    }

    const segments = name.split("_");
    const key = segments[path.length] ?? name;

    const bucket = buckets.get(key);
    if (bucket === undefined) {
      buckets.set(key, [name]);
    } else {
      bucket.push(name);
    }
  }

  const nodes: ParamTreeNode[] = [];

  for (const key of [...buckets.keys()].sort(compareNames)) {
    const under = buckets.get(key) ?? [];
    const childPath = [...path, key];
    const childPrefix = childPath.join("_");

    if (under.length < 2) {
      // A group of one is not a branch: the parameter stays where it is.
      for (const name of under) {
        nodes.push({ label: name, prefix: name, parameter: name, children: [] });
      }

      continue;
    }

    nodes.push({
      label: childPrefix,
      prefix: childPrefix,
      parameter: under.find((name) => name === childPrefix) ?? null,
      children: groupNames(under, childPath),
    });
  }

  return nodes;
}

/**
 * Builds the tree of the parameters.
 *
 * The names are sorted and made unique first, so that the tree does not depend on the
 * order the vehicle happened to report them in.
 */
export function buildParamTree(names: readonly string[]): ParamTreeNode {
  const sorted = [...new Set(names)].sort(compareNames);

  return {
    label: ROOT_LABEL,
    prefix: "",
    parameter: null,
    children: groupNames(sorted, []),
  };
}

/**
 * Whether a parameter belongs to a node of the tree.
 *
 * The root holds every parameter. A branch holds the parameters under its prefix and the
 * parameter that has the prefix as its name.
 */
export function isUnder(node: ParamTreeNode, name: string): boolean {
  if (node.prefix === "") {
    return true;
  }

  return name === node.prefix || name.startsWith(`${node.prefix}_`);
}

/** The node with a prefix, when the tree has one. */
export function findNode(
  tree: ParamTreeNode,
  prefix: string,
): ParamTreeNode | null {
  if (tree.prefix === prefix) {
    return tree;
  }

  for (const child of tree.children) {
    const found = findNode(child, prefix);
    if (found !== null) {
      return found;
    }
  }

  return null;
}

/**
 * The nodes under a node, in the order they are drawn.
 *
 * A branch that is not open does not show its children.
 */
function descendants(
  node: ParamTreeNode,
  open: ReadonlySet<string>,
): readonly ParamTreeNode[] {
  const nodes: ParamTreeNode[] = [];

  for (const child of node.children) {
    nodes.push(child);

    if (child.children.length > 0 && open.has(child.prefix)) {
      nodes.push(...descendants(child, open));
    }
  }

  return nodes;
}

/**
 * The nodes of the tree that are on the screen, in the order they are drawn.
 */
export function flattenTree(
  tree: ParamTreeNode,
  open: ReadonlySet<string>,
): readonly ParamTreeNode[] {
  return [tree, ...descendants(tree, open)];
}

/**
 * The prefixes that have to be open for a node to be seen.
 *
 * The root is always open, and every branch on the way to the node is open too.
 */
export function openPath(tree: ParamTreeNode, prefix: string): readonly string[] {
  const path: string[] = [];

  const walk = (node: ParamTreeNode): boolean => {
    if (node.prefix === prefix) {
      return true;
    }

    for (const child of node.children) {
      if (walk(child)) {
        path.push(child.prefix);
        return true;
      }
    }

    return false;
  };

  walk(tree);
  path.reverse();

  return path;
}
