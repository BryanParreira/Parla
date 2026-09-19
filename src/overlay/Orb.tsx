import { useEffect, useRef } from "react";
import { orbitsFrame, waveFrame, type Dot } from "./orbEngine";

const TINTS = {
  dictate: [255, 255, 255],
  command: [255, 196, 140],
} as const;

/**
 * Dotted sphere drawn on a canvas. While listening its rings ripple harder and faster
 * the louder the voice (`level`, 0–1); while `busy` it switches to particles on orbits.
 */
export default function Orb({ level, busy = false, command = false, size = 36 }: {
  level: number;
  busy?: boolean;
  command?: boolean;
  size?: number;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  // Read by the animation loop every frame, so a new level never restarts it.
  const target = useRef(level);
  target.current = level;

  useEffect(() => {
    const element = canvas.current;
    const ctx = element?.getContext("2d");
    if (!element || !ctx) return;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    element.width = Math.round(size * dpr);
    element.height = Math.round(size * dpr);
    const [r, g, b] = TINTS[command ? "command" : "dictate"];

    const paint = (dots: Dot[]) => {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, size, size);
      for (const dot of dots) {
        const light = Math.min(1, Math.max(0, dot.light));
        ctx.fillStyle = `rgba(${Math.round(r * light)},${Math.round(g * light)},${Math.round(b * light)},${dot.alpha})`;
        ctx.beginPath();
        ctx.arc(dot.x, dot.y, dot.r, 0, Math.PI * 2);
        ctx.fill();
      }
    };

    let frame = 0;
    let last = performance.now();
    let phase = 0;
    let energy = 0;
    const start = last;

    const tick = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      const time = (now - start) / 1000;
      if (busy) {
        paint(orbitsFrame(size, time));
      } else {
        // Rise quickly with the voice, settle slowly after it, so words read as pulses.
        const goal = target.current;
        energy += (goal - energy) * (goal > energy ? 0.35 : 0.08);
        // Integrating the phase lets the tempo follow the voice without the wave jumping.
        phase += dt * (1 + 2.2 * energy);
        paint(waveFrame(size, time, phase, energy));
      }
      frame = requestAnimationFrame(tick);
    };

    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      paint(busy ? orbitsFrame(size, 0.6) : waveFrame(size, 0.6, 0.6, 0));
      return;
    }
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [busy, command, size]);

  return <canvas ref={canvas} aria-hidden className="shrink-0" style={{ width: size, height: size }} />;
}
