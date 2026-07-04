/**
 * Thin wrapper over lucide-react so every icon renders at a consistent stroke
 * and inherits `currentColor` (SPEC §3.3: color = currentColor + the upstream
 * foregroundStyle set by the parent).
 */

import type { LucideIcon } from "lucide-react";

interface IconProps {
  icon: LucideIcon;
  size?: number;
  strokeWidth?: number;
  /** Fill color; `currentColor` renders the filled (`.fill`) SF-Symbol variant. */
  fill?: string;
}

export function Icon({ icon: LucideComp, size = 14, strokeWidth = 2, fill }: IconProps) {
  return <LucideComp size={size} strokeWidth={strokeWidth} fill={fill} />;
}
