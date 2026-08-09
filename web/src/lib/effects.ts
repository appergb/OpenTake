import type { Effect } from "./types";

export const EFFECT_REGISTRY = [
  { name: "grayscale", labelKey: "inspector.effects.grayscale" },
  { name: "sepia", labelKey: "inspector.effects.sepia" },
  { name: "invert", labelKey: "inspector.effects.invert" },
] as const;

export type AdvertisedEffectName = (typeof EFFECT_REGISTRY)[number]["name"];

export function isAdvertisedEffectName(name: string): name is AdvertisedEffectName {
  return EFFECT_REGISTRY.some((effect) => effect.name === name);
}

export function newAdvertisedEffect(name: AdvertisedEffectName): Effect {
  return { name, params: {}, enabled: true };
}
