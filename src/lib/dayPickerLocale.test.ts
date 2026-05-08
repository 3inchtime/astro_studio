import { describe, expect, it } from "vitest";
import { getDayPickerLocale } from "./dayPickerLocale";

describe("day picker locale helpers", () => {
  it("maps supported app languages to localized DayPicker month and weekday labels", () => {
    const may = 4;
    const sunday = 0;

    expect(getDayPickerLocale("zh-CN").localize.month(may, { width: "wide" })).toBe("五月");
    expect(getDayPickerLocale("zh-CN").localize.day(sunday, { width: "short" })).toBe("日");
    expect(getDayPickerLocale("ja").localize.month(may, { width: "wide" })).toBe("5月");
    expect(getDayPickerLocale("ko").localize.month(may, { width: "wide" })).toBe("5월");
    expect(getDayPickerLocale("es").localize.month(may, { width: "wide" })).toBe("mayo");
    expect(getDayPickerLocale("fr").localize.month(may, { width: "wide" })).toBe("mai");
    expect(getDayPickerLocale("de").localize.day(sunday, { width: "short" })).toBe("So");
  });

  it("normalizes regional language codes before choosing the DayPicker locale", () => {
    expect(getDayPickerLocale("zh-HK").code).toBe("zh-TW");
    expect(getDayPickerLocale("en-US").code).toBe("en-US");
    expect(getDayPickerLocale("pt-BR").code).toBe("en-US");
  });
});
