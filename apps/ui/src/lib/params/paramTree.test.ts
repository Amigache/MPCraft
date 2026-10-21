import { describe, expect, it } from "vitest";

import {
  buildParamTree,
  findNode,
  flattenTree,
  isUnder,
  openPath,
  type ParamTreeNode,
} from "./paramTree";

/** The names of a vehicle that has a few parameters of each kind. */
const NAMES = [
  "ARMING_CHECK",
  "ARMING_OPTIONS",
  "COMPASS_OFS_X",
  "FENCE_ENABLE",
  "FENCE_MARGIN",
  "FENCE_TYPE",
  "FS_BATT_ENABLE",
  "RTL_ALT",
  "RTL_SPEED",
  "SERIAL0_BAUD",
  "WP_RADIUS",
];

/** The label of a node, and the labels under it. */
function shape(node: ParamTreeNode): string {
  const children = node.children.map((child) => shape(child)).join(", ");

  return node.children.length === 0
    ? node.label
    : `${node.label} [${children}]`;
}

describe("the tree of the parameters", () => {
  it("has every parameter under one root", () => {
    const tree = buildParamTree(NAMES);

    expect(tree.label).toBe("All");
    expect(tree.prefix).toBe("");
    expect(tree.children.length).toBeGreaterThan(0);
  });

  it("groups the parameters by their common prefix", () => {
    const tree = buildParamTree(NAMES);

    expect(tree.children.map((node) => node.label)).toEqual([
      "ARMING",
      "COMPASS_OFS_X",
      "FENCE",
      "FS_BATT_ENABLE",
      "RTL",
      "SERIAL0_BAUD",
      "WP_RADIUS",
    ]);
  });

  it("keeps the parameters of a branch under it", () => {
    const tree = buildParamTree(NAMES);
    const arming = findNode(tree, "ARMING");

    expect(arming).not.toBeNull();
    expect(arming?.children.map((node) => node.label)).toEqual([
      "ARMING_CHECK",
      "ARMING_OPTIONS",
    ]);
  });

  it("leaves a parameter that has no neighbour under its prefix where it is", () => {
    const tree = buildParamTree(NAMES);

    // WP_RADIUS is the only parameter that begins with WP.
    expect(findNode(tree, "WP")).toBeNull();
    expect(tree.children.some((node) => node.label === "WP_RADIUS")).toBe(true);
  });

  it("nests a parameter whose name is the prefix of another one", () => {
    const tree = buildParamTree(["RTL", "RTL_ALT", "RTL_SPEED"]);

    const rtl = findNode(tree, "RTL");
    expect(rtl).not.toBeNull();
    expect(rtl?.parameter).toBe("RTL");
    expect(rtl?.children.map((node) => node.label)).toEqual([
      "RTL_ALT",
      "RTL_SPEED",
    ]);
  });

  it("nests the branches as well", () => {
    const tree = buildParamTree(["ABC_DEF", "ABC_DEF_GHI", "ABC_JKL"]);

    expect(shape(tree)).toBe("All [ABC [ABC_DEF [ABC_DEF_GHI], ABC_JKL]]");
  });

  it("does not depend on the order the names come in", () => {
    const forwards = buildParamTree(NAMES);
    const backwards = buildParamTree([...NAMES].reverse());

    expect(shape(backwards)).toBe(shape(forwards));
  });

  it("is the same tree when a name is repeated", () => {
    const once = buildParamTree(NAMES);
    const twice = buildParamTree([...NAMES, "RTL_ALT"]);

    expect(shape(twice)).toBe(shape(once));
  });
});

describe("what a branch holds", () => {
  const tree = buildParamTree(NAMES);

  it("holds every parameter when it is the root", () => {
    for (const name of NAMES) {
      expect(isUnder(tree, name)).toBe(true);
    }
  });

  it("holds the parameters under its prefix", () => {
    const rtl = findNode(tree, "RTL");

    expect(rtl).not.toBeNull();
    expect(isUnder(rtl as ParamTreeNode, "RTL_ALT")).toBe(true);
    expect(isUnder(rtl as ParamTreeNode, "RTL_SPEED")).toBe(true);
    expect(isUnder(rtl as ParamTreeNode, "FENCE_ENABLE")).toBe(false);
  });

  it("holds the parameter that has its prefix as its name", () => {
    const nested = buildParamTree(["RTL", "RTL_ALT"]);
    const rtl = findNode(nested, "RTL");

    expect(isUnder(rtl as ParamTreeNode, "RTL")).toBe(true);
    expect(isUnder(rtl as ParamTreeNode, "RTL_ALT")).toBe(true);
  });

  it("holds nothing else", () => {
    const leaf = findNode(tree, "WP_RADIUS");

    expect(isUnder(leaf as ParamTreeNode, "WP_RADIUS")).toBe(true);
    expect(isUnder(leaf as ParamTreeNode, "ARMING_CHECK")).toBe(false);
  });

  it("holds the parameters under a branch that has the same name", () => {
    const nested = buildParamTree(["WP_RADIUS", "WP_RADIUS_2"]);
    const branch = findNode(nested, "WP_RADIUS");

    expect(branch).not.toBeNull();
    expect(isUnder(branch as ParamTreeNode, "WP_RADIUS")).toBe(true);
    expect(isUnder(branch as ParamTreeNode, "WP_RADIUS_2")).toBe(true);
  });
});

describe("the nodes that are on the screen", () => {
  const tree = buildParamTree(NAMES);

  it("are the root and the branches that are closed", () => {
    const visible = flattenTree(tree, new Set());

    expect(visible.map((node) => node.label)).toEqual([
      "All",
      "ARMING",
      "COMPASS_OFS_X",
      "FENCE",
      "FS_BATT_ENABLE",
      "RTL",
      "SERIAL0_BAUD",
      "WP_RADIUS",
    ]);
  });

  it("are the leaves of a branch that is open", () => {
    const visible = flattenTree(tree, new Set(["ARMING"]));

    expect(visible.map((node) => node.label)).toEqual([
      "All",
      "ARMING",
      "ARMING_CHECK",
      "ARMING_OPTIONS",
      "COMPASS_OFS_X",
      "FENCE",
      "FS_BATT_ENABLE",
      "RTL",
      "SERIAL0_BAUD",
      "WP_RADIUS",
    ]);
  });

  it("are the leaves of a branch that is open, once each", () => {
    const visible = flattenTree(tree, new Set(["ARMING"]));
    const labels = visible.map((node) => node.label);

    expect(new Set(labels).size).toBe(labels.length);
  });

  it("tell which branches have to be open for a node to be seen", () => {
    expect(openPath(tree, "")).toEqual([]);
    expect(openPath(tree, "ARMING")).toEqual(["ARMING"]);
    expect(openPath(tree, "ARMING_CHECK")).toEqual(["ARMING", "ARMING_CHECK"]);
  });
});
