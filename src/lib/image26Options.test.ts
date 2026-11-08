import { describe, expect, it } from "vitest";
import { image26ValidationError } from "./image26Options";

const defaults = {
  model: "gpt-image-2.6-flare",
  format: "png",
  size: "auto",
  count: 1,
  background: "auto",
  editing: false,
  options: {},
};
describe("2.6 request constraints", () => {
  it("blocks SVG edits and multiple SVG outputs", () => {
    expect(
      image26ValidationError({ ...defaults, format: "svg", editing: true }),
    ).toBe("svgGenerateOnly");
    expect(
      image26ValidationError({ ...defaults, format: "svg", count: 2 }),
    ).toBe("svgSingle");
  });
  it("validates GIF frame parameters without applying raster minimum pixels", () => {
    expect(
      image26ValidationError({
        ...defaults,
        format: "gif",
        size: "512x512",
        options: { frameCount: 16, frameDurationMs: 2000 },
      }),
    ).toBeNull();
    expect(
      image26ValidationError({
        ...defaults,
        format: "gif",
        options: { frameCount: 17 },
      }),
    ).toBe("gifFramesInvalid");
    expect(
      image26ValidationError({
        ...defaults,
        format: "gif",
        options: { frameDurationMs: 19 },
      }),
    ).toBe("gifDurationInvalid");
    expect(
      image26ValidationError({
        ...defaults,
        format: "gif",
        options: { frameCount: NaN },
      }),
    ).toBe("gifFramesInvalid");
  });
  it("checks custom raster dimensions and transparent output compatibility", () => {
    expect(
      image26ValidationError({ ...defaults, size: "1536x864" }),
    ).toBeNull();
    expect(
      image26ValidationError({
        ...defaults,
        format: "svg",
        background: "transparent",
      }),
    ).toBeNull();
    expect(image26ValidationError({ ...defaults, size: "512x512" })).toBe(
      "sizeInvalid",
    );
    expect(image26ValidationError({ ...defaults, size: "1025x1024" })).toBe(
      "sizeInvalid",
    );
    expect(
      image26ValidationError({
        ...defaults,
        format: "gif",
        background: "transparent",
      }),
    ).toBe("transparentFormat");
  });
  it("rejects conflicting annotation inputs", () => {
    expect(
      image26ValidationError({
        ...defaults,
        editing: true,
        options: { markupPath: "markup.png", maskPath: "mask.png" },
      }),
    ).toBe("overlayConflict");
  });
});

it("rejects invalid JPEG/WebP compression", () => {
  for (const compression of [-1, 101, 1.5, NaN])
    expect(
      image26ValidationError({ ...defaults, format: "webp", compression }),
    ).toBe("compressionInvalid");
  expect(
    image26ValidationError({ ...defaults, format: "webp", compression: 0 }),
  ).toBeNull();
});

it("accepts the full GIF API duration range beyond the guide examples", () => {
  for (const frameDurationMs of [2001, 655350, 4294967295])
    expect(
      image26ValidationError({
        ...defaults,
        format: "gif",
        options: { frameDurationMs },
      }),
    ).toBeNull();
  expect(
    image26ValidationError({
      ...defaults,
      format: "gif",
      options: { frameDurationMs: 4294967296 },
    }),
  ).toBe("gifDurationInvalid");
});
