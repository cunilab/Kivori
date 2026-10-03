import '@testing-library/jest-dom/vitest';

// jsdom has no canvas backend and (in some versions) no `ImageData` constructor. The blit path only
// needs `ImageData` to carry bytes, so provide a minimal polyfill — this keeps canvas tests free of a
// native `node-canvas` dependency while still exercising the real blit code path.
if (typeof globalThis.ImageData === 'undefined') {
  class ImageDataPolyfill {
    readonly data: Uint8ClampedArray;
    readonly width: number;
    readonly height: number;
    readonly colorSpace: PredefinedColorSpace = 'srgb';
    constructor(data: Uint8ClampedArray, width: number, height: number) {
      this.data = data;
      this.width = width;
      this.height = height;
    }
  }
  globalThis.ImageData = ImageDataPolyfill as unknown as typeof ImageData;
}

// jsdom has no matchMedia (the sidebar's mobile check and the theme read it) and no ResizeObserver
// (Base UI popups observe their anchors). Desktop-width, no-op stand-ins are enough for these tests.
if (typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string): MediaQueryList =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  } as unknown as typeof ResizeObserver;
}
// Base UI dispatches PointerEvents and awaits element animations; jsdom implements neither.
if (typeof window.PointerEvent === 'undefined') {
  window.PointerEvent = class extends MouseEvent {
    readonly pointerId: number;
    readonly pointerType: string;
    constructor(type: string, init: PointerEventInit = {}) {
      super(type, init);
      this.pointerId = init.pointerId ?? 0;
      this.pointerType = init.pointerType ?? 'mouse';
    }
  } as unknown as typeof PointerEvent;
}
if (typeof Element.prototype.getAnimations !== 'function') {
  Element.prototype.getAnimations = () => [];
}
