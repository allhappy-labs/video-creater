/**
 * Paint order inside a track lane, which isolates its stacking context. Clip fills, filmstrips and
 * waveforms paint at the base; the transition window and its edge grips tint over them; clip labels
 * paint over that overlay so its lines never cross a label; controls (transition badge and edge
 * handles, trim handles) paint on top. Clips themselves open no stacking context, so their labels
 * join this order.
 */
export const laneLayerClass = {
  transitionOverlay: "z-[1]",
  clipLabel: "z-[2]",
  controls: "z-20",
} as const;
