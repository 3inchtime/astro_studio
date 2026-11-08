function rgb(hex: string): number[] {
  return [1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16));
}

function luminance(hex: string): number {
  const values = rgb(hex).map((value) => {
    const channel = value / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
}

export function contrastRatio(a: string, b: string): number {
  if (!a || !b) return 0;
  const first = luminance(a), second = luminance(b);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

/** Keep the preset's hue, moving toward ink/paper only as far as required. */
export function readableColor(color: string, backgrounds: string[], target: string, ratio: number): string {
  const start = rgb(color), end = rgb(target);
  for (let step = 0; step <= 100; step++) {
    const mixed = "#" + start.map((value, index) => Math.round(value + (end[index] - value) * step / 100).toString(16).padStart(2, "0")).join("");
    if (backgrounds.every((background) => contrastRatio(mixed, background) >= ratio)) return mixed;
  }
  return target;
}
