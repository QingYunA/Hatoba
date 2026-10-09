import { describe, expect, it } from "vitest";
import { defaultTerminalFont } from "./platform";

describe("defaultTerminalFont", () => {
  it("is a font each platform has", () => {
    expect(defaultTerminalFont("macos")).toBe("Menlo");
    expect(defaultTerminalFont("windows")).toBe("Cascadia Mono");
    expect(defaultTerminalFont("linux")).toBe("Cascadia Mono");
  });
});
