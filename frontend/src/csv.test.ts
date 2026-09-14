import { describe, expect, it } from "vitest";
import { toCsv } from "./csv";

describe("toCsv", () => {
  it("renders a header row from the given columns", () => {
    const csv = toCsv([{ a: 1, b: 2 }], ["a", "b"]);
    expect(csv.split("\n")[0]).toBe("a,b");
  });

  it("renders one row per item, in column order", () => {
    const csv = toCsv(
      [
        { name: "XLM", amount: 100 },
        { name: "USDC", amount: 250 },
      ],
      ["name", "amount"],
    );
    expect(csv).toBe("name,amount\nXLM,100\nUSDC,250");
  });

  it("renders null/undefined as an empty cell", () => {
    const csv = toCsv([{ a: null, b: undefined }], ["a", "b"]);
    expect(csv).toBe("a,b\n,");
  });

  it("quotes and escapes values containing commas, quotes, or newlines", () => {
    const csv = toCsv([{ msg: 'has a "quote", a comma, and a\nnewline' }], ["msg"]);
    expect(csv).toBe('msg\n"has a ""quote"", a comma, and a\nnewline"');
  });

  it("only includes the requested columns, even if the row has more", () => {
    const csv = toCsv([{ a: 1, b: 2, c: 3 }], ["a", "c"]);
    expect(csv).toBe("a,c\n1,3");
  });
});
