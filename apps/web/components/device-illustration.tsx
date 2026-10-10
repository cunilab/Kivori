import { useId, type ReactElement } from 'react';

// Front view of the Kivori wedge, drawn on a 3 px grid. Colours are the product's own (light matte shell,
// black visor and knob, keycap-green buttons), so the illustration looks the same in both site themes.
const FX = 15;
const FY = 10;
const unit = (n: number): number => n * 3;

const BUTTONS = [
  { x: 65, rotate: -8 },
  { x: 82, rotate: 0 },
  { x: 99, rotate: 8 },
] as const;

interface DeviceIllustrationProps {
  /** Accessible description. Leave out when the surrounding text already names the device. */
  label?: string;
  className?: string;
}

export function DeviceIllustration({ label, className }: DeviceIllustrationProps): ReactElement {
  const id = useId().replace(/:/g, '');
  const knobX = FX + unit(27);
  const knobY = FY + unit(35);
  const screenX = FX + unit(82) - unit(23.4) / 2;
  const screenY = FY + unit(23.4) - unit(23.4) / 2;
  const screen = unit(23.4);
  const a11y = label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true };

  return (
    <svg viewBox="0 0 360 262" className={className} {...a11y}>
      <defs>
        <linearGradient id={`${id}-shell`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#f4f6fa" />
          <stop offset="1" stopColor="#dde1ea" />
        </linearGradient>
        <radialGradient id={`${id}-knob`} cx="0.35" cy="0.3" r="0.9">
          <stop offset="0" stopColor="#343b52" />
          <stop offset="1" stopColor="#10131d" />
        </radialGradient>
        <linearGradient id={`${id}-cap`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#7df2c4" />
          <stop offset="1" stopColor="#4fcf9f" />
        </linearGradient>
      </defs>

      <ellipse cx="180" cy="248" rx="150" ry="9" fill="#000" opacity="0.28" />
      <rect x={FX + 20} y="224" width="290" height="14" rx="6" fill="#12151f" />
      <rect x={FX} y={FY + 16} width="330" height="210" rx="24" fill="#aab1c1" />
      <rect x={FX} y={FY} width="330" height="210" rx="24" fill={`url(#${id}-shell)`} />
      <rect
        x={FX + 4}
        y={FY + 4}
        width="322"
        height="202"
        rx="20"
        fill="none"
        stroke="#fff"
        strokeOpacity="0.7"
      />

      <rect
        x={FX + unit(54)}
        y={FY + unit(6)}
        width={unit(54)}
        height={unit(58)}
        rx="14"
        fill="#12151f"
      />
      <circle cx={knobX} cy={knobY} r="76" fill="#12151f" />
      <circle cx={knobX} cy={knobY} r="72" fill={`url(#${id}-knob)`} />
      <circle
        cx={knobX}
        cy={knobY}
        r="69"
        fill="none"
        stroke="#454d66"
        strokeWidth="3"
        strokeDasharray="1.5 4"
      />
      <circle
        cx={knobX}
        cy={knobY}
        r="58"
        fill="none"
        stroke="#fff"
        strokeOpacity="0.07"
        strokeWidth="2"
      />
      <circle cx={knobX - 33} cy={knobY - 33} r="5.5" fill="#e8ecf5" />

      <rect
        x={screenX - 4}
        y={screenY - 4}
        width={screen + 8}
        height={screen + 8}
        rx="9"
        fill="#05070d"
      />
      <rect x={screenX} y={screenY} width={screen} height={screen} rx="6" fill="#0c101c" />
      <rect x={screenX + 18} y={screenY + 21} width="34" height="28" rx="8" fill="#35a87c" />
      <rect x={screenX + 18} y={screenY + 15} width="34" height="26" rx="8" fill="#7df2c4" />
      <rect x={screenX + 26} y={screenY + 22} width="4.5" height="10" rx="2.25" fill="#0c101c" />
      <rect x={screenX + 39.5} y={screenY + 22} width="4.5" height="10" rx="2.25" fill="#0c101c" />
      <circle cx={screenX + screen - 8} cy={screenY + 8} r="3" fill="#3cc47c" />

      {BUTTONS.map(({ x, rotate }) => {
        const cx = FX + unit(x);
        const cy = FY + unit(52);
        return (
          <g key={x} transform={`rotate(${rotate} ${cx} ${cy})`}>
            <rect x={cx - 21} y={cy - 21} width="42" height="42" rx="9" fill="#05070d" />
            <rect x={cx - 17} y={cy - 17} width="34" height="34" rx="7" fill={`url(#${id}-cap)`} />
          </g>
        );
      })}
    </svg>
  );
}
