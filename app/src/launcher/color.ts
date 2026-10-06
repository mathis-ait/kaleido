/** Couleur dominante (vive) d'une jaquette, pour le halo et le fond du lanceur. */

const cache = new Map<string, Promise<string | null>>();

export function dominantColor(url: string): Promise<string | null> {
  let p = cache.get(url);
  if (!p) {
    p = new Promise((resolve) => {
      const img = new Image();
      img.crossOrigin = "anonymous";
      img.onload = () => {
        try {
          const size = 24;
          const canvas = document.createElement("canvas");
          canvas.width = canvas.height = size;
          const g = canvas.getContext("2d", { willReadFrequently: true })!;
          g.drawImage(img, 0, 0, size, size);
          const px = g.getImageData(0, 0, size, size).data;
          // Histogramme des teintes (par tranches de 15°), pondéré par la saturation et la
          // luminosité : la teinte la plus présente l'emporte, le blanc de la boîte ne compte pas.
          const BUCKETS = 24;
          const weights = new Array(BUCKETS).fill(0);
          const sums = Array.from({ length: BUCKETS }, () => [0, 0, 0]);
          for (let i = 0; i < px.length; i += 4) {
            const [r, gr, b] = [px[i], px[i + 1], px[i + 2]];
            const max = Math.max(r, gr, b);
            const min = Math.min(r, gr, b);
            const sat = max === 0 ? 0 : (max - min) / max;
            if (sat < 0.3 || max < 50) continue;
            const bucket = Math.floor((hue(r, gr, b) / 360) * BUCKETS) % BUCKETS;
            const weight = sat * sat * (max / 255);
            weights[bucket] += weight;
            sums[bucket][0] += r * weight;
            sums[bucket][1] += gr * weight;
            sums[bucket][2] += b * weight;
          }
          let best = 0;
          for (let i = 1; i < BUCKETS; i++) if (weights[i] > weights[best]) best = i;
          if (weights[best] === 0) return resolve(null);
          const w = weights[best];
          resolve(boost(sums[best][0] / w, sums[best][1] / w, sums[best][2] / w));
        } catch {
          resolve(null);
        }
      };
      img.onerror = () => resolve(null);
      img.src = url;
    });
    cache.set(url, p);
  }
  return p;
}

function hue(r: number, g: number, b: number): number {
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  if (d === 0) return 0;
  let h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  h *= 60;
  return h < 0 ? h + 360 : h;
}

/** Ravive la couleur (saturation et luminosité minimales) pour qu'elle brille sur fond sombre. */
function boost(r: number, g: number, b: number): string {
  const max = Math.max(r, g, b) / 255;
  const min = Math.min(r, g, b) / 255;
  const l = (max + min) / 2;
  let h = 0;
  const d = max - min;
  if (d > 0) {
    const R = r / 255, G = g / 255, B = b / 255;
    if (max === R) h = ((G - B) / d) % 6;
    else if (max === G) h = (B - R) / d + 2;
    else h = (R - G) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  return `hsl(${Math.round(h)} ${Math.round(Math.max(s, 0.65) * 100)}% ${Math.round(Math.min(Math.max(l, 0.55), 0.68) * 100)}%)`;
}
