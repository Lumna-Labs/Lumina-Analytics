import { describe, expect, it } from "vitest";
import {
  assetLabel,
  fmtCompact,
  fmtPct,
  fmtPrecise,
  fmtRelative,
  fmtTime,
  fmtUsd,
  pairLabel,
  searchResultHref,
  truncateMiddle,
} from "./format";

describe("fmtCompact", () => {
  it("formats numbers compactly", () => {
    expect(fmtCompact(1_500_000)).toBe("1.5M");
  });

  it("parses numeric strings", () => {
    expect(fmtCompact("2500")).toBe("2.5K");
  });

  it("returns an em dash for null/undefined/NaN", () => {
    expect(fmtCompact(null)).toBe("—");
    expect(fmtCompact(undefined)).toBe("—");
    expect(fmtCompact("not-a-number")).toBe("—");
  });
});

describe("fmtPrecise", () => {
  it("formats with up to two decimal places", () => {
    expect(fmtPrecise(1234.5)).toBe("1,234.5");
  });

  it("returns an em dash for missing values", () => {
    expect(fmtPrecise(null)).toBe("—");
  });
});

describe("fmtUsd", () => {
  it("formats a USD amount compactly", () => {
    expect(fmtUsd(1_500_000)).toBe("$1.5M");
  });

  it("returns null (not a placeholder string) for missing values", () => {
    expect(fmtUsd(null)).toBeNull();
    expect(fmtUsd(undefined)).toBeNull();
    expect(fmtUsd("nope")).toBeNull();
  });
});

describe("fmtPct", () => {
  it("adds a plus sign for positive values", () => {
    expect(fmtPct(12.345)).toBe("+12.35%");
  });

  it("leaves negative values as-is", () => {
    expect(fmtPct(-5)).toBe("-5.00%");
  });

  it("returns an em dash for missing values", () => {
    expect(fmtPct(null)).toBe("—");
  });
});

describe("assetLabel", () => {
  it("renders native as XLM", () => {
    expect(assetLabel("native")).toBe("XLM");
  });

  it("renders the code portion of CODE:ISSUER", () => {
    expect(assetLabel("USDC:GISSUER123")).toBe("USDC");
  });
});

describe("pairLabel", () => {
  it("joins two asset labels", () => {
    expect(pairLabel("native", "USDC:GISSUER")).toBe("XLM / USDC");
  });
});

describe("truncateMiddle", () => {
  it("leaves short strings untouched", () => {
    expect(truncateMiddle("GSHORT")).toBe("GSHORT");
  });

  it("truncates long strings with an ellipsis in the middle", () => {
    const long = "GABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";
    expect(truncateMiddle(long)).toBe("GABCDE…567890");
  });
});

describe("searchResultHref", () => {
  it("links a pool result to its detail page", () => {
    expect(searchResultHref({ result_type: "pool", key: "POOL123" })).toBe("/pools/POOL123");
  });

  it("splits a token key into code and issuer", () => {
    expect(searchResultHref({ result_type: "token", key: "USDC:GISSUER" })).toBe(
      "/tokens/USDC/GISSUER",
    );
  });

  it("links an account result to the activity page", () => {
    expect(searchResultHref({ result_type: "account", key: "GACCOUNT" })).toBe(
      "/accounts/GACCOUNT",
    );
  });
});

describe("fmtTime / fmtRelative", () => {
  it("return an em dash for missing timestamps", () => {
    expect(fmtTime(null)).toBe("—");
    expect(fmtRelative(undefined)).toBe("—");
  });

  it("fmtRelative reports seconds/minutes/hours/days ago", () => {
    const now = Date.now();
    expect(fmtRelative(new Date(now - 5_000).toISOString())).toBe("5s ago");
    expect(fmtRelative(new Date(now - 5 * 60_000).toISOString())).toBe("5m ago");
    expect(fmtRelative(new Date(now - 5 * 3_600_000).toISOString())).toBe("5h ago");
    expect(fmtRelative(new Date(now - 5 * 86_400_000).toISOString())).toBe("5d ago");
  });
});
