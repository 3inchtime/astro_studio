import { afterEach, describe, expect, it, vi } from "vitest";
import { exportCanvasFrame, loadImage } from "./export";
import {
  createCanvasDocumentContent,
  createImageObject,
  createStrokeObject,
} from "./document";
vi.mock("../api", () => ({ toAssetUrl: (p: string) => `asset:${p}` }));
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});
describe("canvas frame export", () => {
  it("rejects unavailable rendering context", async () => {
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    await expect(
      exportCanvasFrame(createCanvasDocumentContent()),
    ).rejects.toThrow();
  });
  it("waits for images and converts native asset paths", async () => {
    vi.useFakeTimers();
    let img: HTMLImageElement;
    vi.spyOn(window, "Image").mockImplementation(function () {
      img = {
        set src(value: string) {
          expect(value).toBe("asset:/pic.png");
          expect(img.crossOrigin).toBe("anonymous");
        },
        onload: null,
        onerror: null,
      } as unknown as HTMLImageElement;
      return img;
    });
    let done = false;
    const p = loadImage("/pic.png").then((v) => {
      done = true;
      return v;
    });
    await vi.advanceTimersByTimeAsync(100);
    expect(done).toBe(false);
    img!.onload!(new Event("load"));
    expect(await p).toBe(img!);
  });
  it("rejects failed image loads", async () => {
    vi.spyOn(window, "Image").mockImplementation(function () {
      const img = {
        set src(_value: string) {
          queueMicrotask(() => img.onerror?.(new Event("error")));
        },
        onload: null,
        onerror: null,
      } as unknown as HTMLImageElement;
      return img;
    });
    await expect(loadImage("/bad.png")).rejects.toThrow();
  });
  it("composites each layer independently and rotates images around their top left", async () => {
    const contexts: any[] = [];
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
      () => {
        const c = {
          fillRect: vi.fn(),
          save: vi.fn(),
          restore: vi.fn(),
          translate: vi.fn(),
          rotate: vi.fn(),
          drawImage: vi.fn(),
          beginPath: vi.fn(),
          moveTo: vi.fn(),
          lineTo: vi.fn(),
          stroke: vi.fn(),
        };
        contexts.push(c);
        return c as any;
      },
    );
    vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue(
      "data:image/png;base64,ok",
    );
    vi.spyOn(window, "Image").mockImplementation(function () {
      const img = {
        set src(_v: string) {
          queueMicrotask(() => img.onload?.(new Event("load")));
        },
        onload: null,
        onerror: null,
      } as unknown as HTMLImageElement;
      return img;
    });
    const c = createCanvasDocumentContent();
    c.frame.x = 5;
    c.frame.y = 6;
    c.layers[0].objects = [
      createImageObject({
        image_path: "/p.png",
        x: 20,
        y: 30,
        width: 100,
        height: 80,
        rotation: 90,
      }),
      createStrokeObject({ tool: "eraser", points: [0, 0, 10, 10] }),
    ];
    await exportCanvasFrame(c);
    expect(contexts.length).toBe(2);
    expect(contexts[1].translate).toHaveBeenCalledWith(15, 24);
    expect(contexts[1].drawImage).toHaveBeenCalledWith(
      expect.anything(),
      0,
      0,
      100,
      80,
    );
    expect(contexts[0].drawImage).toHaveBeenCalled();
    expect(contexts[0].globalCompositeOperation).not.toBe("destination-out");
  });
});

it("exports Rust-shaped saved rectangle and text despite default points arrays", async () => {
  const ctx = {
    fillRect: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    translate: vi.fn(),
    rotate: vi.fn(),
    drawImage: vi.fn(),
    beginPath: vi.fn(),
    rect: vi.fn(),
    fill: vi.fn(),
    stroke: vi.fn(),
    fillText: vi.fn(),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    ctx as unknown as CanvasRenderingContext2D,
  );
  vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue(
    "data:image/png;base64,ok",
  );
  const c = createCanvasDocumentContent();
  c.layers[0].objects = [
    {
      type: "rectangle",
      id: "r",
      x: 10,
      y: 20,
      width: 40,
      height: 30,
      rotation: 0,
      color: "#000",
      size: 2,
      fill: "#f00",
      opacity: 1,
      points: [],
    },
    {
      type: "text",
      id: "t",
      x: 20,
      y: 40,
      width: 100,
      height: 30,
      rotation: 0,
      text: "Saved",
      fontSize: 24,
      color: "#000",
      opacity: 1,
      points: [],
    },
  ] as unknown as import("../../types").CanvasObject[];
  await exportCanvasFrame(c);
  expect(ctx.rect).toHaveBeenCalledWith(0, 0, 40, 30);
  expect(ctx.fillText).toHaveBeenCalledWith("Saved", 0, 0);
});
it("draws ellipse and arrow paths as visible geometry", async () => {
  const ctx = {
    fillRect: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    translate: vi.fn(),
    rotate: vi.fn(),
    drawImage: vi.fn(),
    beginPath: vi.fn(),
    ellipse: vi.fn(),
    fill: vi.fn(),
    stroke: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    ctx as unknown as CanvasRenderingContext2D,
  );
  vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue(
    "data:image/png;base64,ok",
  );
  const c = createCanvasDocumentContent();
  c.layers[0].objects = [
    {
      type: "ellipse",
      id: "e",
      x: 0,
      y: 0,
      width: 40,
      height: 20,
      rotation: 0,
      color: "#000",
      size: 2,
      fill: "#f00",
      opacity: 1,
    },
    {
      type: "arrow",
      id: "a",
      points: [0, 0, 100, 0],
      size: 2,
      color: "#000",
      opacity: 1,
    },
  ];
  await exportCanvasFrame(c);
  expect(ctx.ellipse).toHaveBeenCalledWith(20, 10, 20, 10, 0, 0, Math.PI * 2);
  expect(ctx.lineTo).toHaveBeenCalledWith(100, 0);
  expect(ctx.lineTo).toHaveBeenCalledWith(
    100 - 12 * Math.cos(Math.PI / 6),
    -12 * Math.sin(Math.PI / 6),
  );
  expect(ctx.stroke).toHaveBeenCalledTimes(3);
});
