import { describe, expect, it } from "vitest";
import { ops, parseTable, serializeTable, splitRow } from "./table";

const md = "| Name | Qty |\n| :--- | ---: |\n| apples | 3 |\n| a \\| b | 10 |";

describe("table model", () => {
  it("parses cells, alignment and escaped pipes", () => {
    const t = parseTable(md);
    expect(t.rows).toEqual([
      ["Name", "Qty"],
      ["apples", "3"],
      ["a | b", "10"],
    ]);
    expect(t.aligns).toEqual(["left", "right"]);
    expect(splitRow("no | outer")).toEqual(["no", "outer"]);
  });
  it("round-trips through Markdown", () => {
    expect(serializeTable(parseTable(md))).toBe(md);
  });
  it("pads short rows and drops extra cells", () => {
    expect(parseTable("| a | b |\n|---|---|\n| 1 |\n| 1 | 2 | 3 |").rows.slice(1)).toEqual([
      ["1", ""],
      ["1", "2"],
    ]);
  });
  it("edits rows and columns", () => {
    const t = parseTable(md);
    expect(ops.insertRow(t, 1).rows[1]).toEqual(["", ""]);
    expect(ops.insertRow(t, 0).rows[0]).toEqual(["Name", "Qty"]); // never above the header
    expect(ops.deleteRow(t, 0)).toBe(t);
    expect(ops.moveRow(t, 1, 1).rows.slice(1).map((r) => r[0])).toEqual(["a | b", "apples"]);
    expect(ops.insertCol(t, 1).rows[0]).toEqual(["Name", "", "Qty"]);
    expect(ops.moveCol(t, 0, 1).aligns).toEqual(["right", "left"]);
    expect(ops.deleteCol(ops.deleteCol(t, 0), 0).aligns).toEqual(["right"]); // keeps the last column
    expect(ops.duplicateCol(t, 1).rows[1]).toEqual(["apples", "3", "3"]);
    expect(ops.align(t, 0, "center").aligns[0]).toBe("center");
  });
  it("sorts body rows naturally, header fixed", () => {
    const t = ops.sort(parseTable(md), 1, -1);
    expect(t.rows.map((r) => r[1])).toEqual(["Qty", "10", "3"]);
  });
});

describe("diff", () => {
  it("finds the changed middle", async () => {
    const { diff } = await import("./table");
    expect(diff("| a | b |", "| ab | b |")).toEqual({ from: 3, to: 3, insert: "b" });
    expect(diff("abc", "abc")).toEqual({ from: 3, to: 3, insert: "" });
    expect(diff("abc", "ac")).toEqual({ from: 1, to: 2, insert: "" });
  });
});
