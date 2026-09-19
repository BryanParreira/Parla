// Geometry for the overlay orb: a dotted sphere, rotated and projected orthographically,
// with depth carried by dot size and brightness alone. The approach follows Jakub
// Antalik's thinking-orbs (MIT, github.com/Jakubantalik/thinking-orbs); the voice-driven
// wave is Parla's own.
//
// Everything here is pure so a frame can be rendered outside the browser to check it.

export type Dot = {
  x: number;
  y: number;
  /** Depth after rotation, -1 (far) to 1 (near). Dots are returned sorted far to near. */
  z: number;
  r: number;
  /** 0 = dim, 1 = full brightness. */
  light: number;
  alpha: number;
};

type Projector = (x: number, y: number, z: number) => [number, number, number];

function projector(yaw: number, tilt: number, cx: number, cy: number): Projector {
  const sy = Math.sin(yaw);
  const cyw = Math.cos(yaw);
  const st = Math.sin(tilt);
  const ct = Math.cos(tilt);
  return (x, y, z) => {
    const x1 = x * cyw + z * sy;
    const z1 = -x * sy + z * cyw;
    const y1 = y * ct - z1 * st;
    const z2 = y * st + z1 * ct;
    return [cx + x1, cy - y1, z2];
  };
}

function hash(a: number, b: number) {
  const h = Math.sin(a * 12.9898 + b * 78.233) * 43758.5453;
  return h - Math.floor(h);
}

// Dot radii are tuned for a 300px orb and shrink sub-linearly, so small orbs stay legible.
const radiusScale = (size: number) => (size / 300) ** 0.6;

const sortFarToNear = (dots: Dot[]) => dots.sort((a, b) => a.z - b.z);

/**
 * Latitude rings whose radii ripple with two waves at different tempi, so the motion
 * never quite repeats. `phase` advances faster while the user is loud; `energy` (0–1)
 * sets how far the rings swell.
 */
export function waveFrame(size: number, time: number, phase: number, energy: number): Dot[] {
  const c = size / 2;
  const R = c * 0.84;
  const project = projector(time * 0.35, 0.38, c, c);
  const rs = radiusScale(size) * 1.35;
  const amplitude = 0.05 + 0.11 * energy;
  const rings = 13;
  const density = 30;

  const dots: Dot[] = [];
  for (let ring = 0; ring <= rings; ring++) {
    const lat = -Math.PI / 2 + (ring / rings) * Math.PI;
    const cosLat = Math.cos(lat);
    const sinLat = Math.sin(lat);
    const wave = 0.62 * Math.sin(phase * 2.1 - ring * 0.52) + 0.38 * Math.sin(phase * 1.27 + ring * 0.83);
    const crest = Math.max(0, wave);
    const radius = R * (0.86 + amplitude * wave);
    const count = Math.max(1, Math.round(Math.abs(cosLat) * density));
    for (let j = 0; j < count; j++) {
      const lon = (j / count) * Math.PI * 2;
      const [x, y, z] = project(cosLat * Math.cos(lon) * radius, sinLat * radius, cosLat * Math.sin(lon) * radius);
      const depth = (z / R + 1) / 2;
      dots.push({
        x,
        y,
        z: z / R,
        r: Math.max(0.35, (0.6 + 1.7 * depth) * (1 + (0.25 + 0.3 * energy) * crest) * rs),
        light: 0.3 + 0.6 * depth + 0.15 * crest,
        alpha: 1,
      });
    }
  }
  return sortFarToNear(dots);
}

/**
 * Particles running on tilted orbits over faint ghost paths — the "working on it" state
 * shown while a transcript is being produced.
 */
export function orbitsFrame(size: number, time: number): Dot[] {
  const c = size / 2;
  const R = c * 0.84;
  const project = projector(time * 0.12, 0.3, c, c);
  const rs = radiusScale(size) * 1.35;
  const orbits = 9;
  const ghosts = 30;
  const particles = 2;

  const dots: Dot[] = [];
  for (let orbit = 0; orbit < orbits; orbit++) {
    const h1 = hash(orbit, 1.7);
    const h2 = hash(orbit, 5.2);
    const h3 = hash(orbit, 8.9);
    const ro = R * (0.45 + 0.52 * h1);
    // Orbit plane: a random normal and two unit vectors perpendicular to it.
    const theta = h1 * Math.PI * 2;
    const phi = Math.acos(2 * h2 - 1);
    const nx = Math.sin(phi) * Math.cos(theta);
    const ny = Math.cos(phi);
    const nz = Math.sin(phi) * Math.sin(theta);
    const ul = Math.max(1e-6, Math.hypot(ny, nx));
    const ux = -ny / ul;
    const uy = nx / ul;
    const vx = -nz * uy;
    const vy = nz * ux;
    const vz = nx * uy - ny * ux;
    const speed = (0.9 + 1.6 * h3) * (h3 > 0.5 ? 1 : -1);
    const point = (a: number) =>
      project(
        (ux * Math.cos(a) + vx * Math.sin(a)) * ro,
        (uy * Math.cos(a) + vy * Math.sin(a)) * ro,
        vz * Math.sin(a) * ro,
      );

    for (let k = 0; k < ghosts; k++) {
      const [x, y, z] = point((k / ghosts) * Math.PI * 2);
      const depth = (z / ro + 1) / 2;
      dots.push({ x, y, z: z / R, r: Math.max(0.3, 0.9 * rs), light: 0.45, alpha: 0.35 * (0.4 + 0.6 * depth) });
    }
    for (let m = 0; m < particles; m++) {
      const [x, y, z] = point(time * speed + (m / particles) * Math.PI * 2 + h2 * 6);
      const depth = (z / ro + 1) / 2;
      dots.push({ x, y, z: z / R, r: Math.max(0.45, (1.0 + 1.3 * depth) * rs), light: 0.75 + 0.25 * depth, alpha: 1 });
    }
  }
  return sortFarToNear(dots);
}
