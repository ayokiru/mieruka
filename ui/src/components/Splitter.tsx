import type { KeyboardEvent, PointerEvent } from "react";

interface Props {
  label: string;
  width: number;
  min: number;
  max: number;
  // +1 when dragging right makes the pane wider (left pane), -1 for a right pane.
  direction: 1 | -1;
  onWidth: (w: number) => void;
}

export function Splitter({ label, width, min, max, direction, onWidth }: Props) {
  const clamp = (w: number) => Math.round(Math.min(max, Math.max(min, w)));

  function onPointerDown(e: PointerEvent<HTMLDivElement>) {
    const startX = e.clientX;
    const startW = width;
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    const move = (ev: globalThis.PointerEvent) => onWidth(clamp(startW + (ev.clientX - startX) * direction));
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    const step = e.shiftKey ? 40 : 10;
    if (e.key === "ArrowLeft") onWidth(clamp(width - step * direction));
    else if (e.key === "ArrowRight") onWidth(clamp(width + step * direction));
    else return;
    e.preventDefault();
  }

  return (
    <div
      className="splitter"
      role="separator"
      aria-label={label}
      aria-orientation="vertical"
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
    />
  );
}
