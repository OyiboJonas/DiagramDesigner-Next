// Phase-1 production SVG presentation boundary.
//
// ADR-019 selected the already measured SVG adapter as the Phase-1 production
// renderer. Keep the evidence-tested implementation behind this stable facade so
// editor/domain code remains renderer-neutral and the implementation can still be
// replaced without changing the application interaction contract.
export {
  createCandidateSvgSurface as createSvgSurface,
  mapClientPointToViewBox,
} from './candidate-svg-surface.mjs';
