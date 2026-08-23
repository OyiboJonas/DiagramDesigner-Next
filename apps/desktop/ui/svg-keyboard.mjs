// Phase-1 production SVG keyboard/focus boundary.
//
// The measured candidate implementation remains unchanged behind this facade;
// renderer-independent keyboard semantics stay outside SVG-specific DOM code.
export {
  createCandidateSvgKeyboardSurface as createSvgKeyboardSurface,
} from './candidate-svg-keyboard.mjs';
