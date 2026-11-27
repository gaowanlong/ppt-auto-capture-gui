import * as native from 'libppt_capture_ohos_core.so';

export interface FrameStatus {
  state: string;
  saved: boolean;
  black: boolean;
  transition: boolean;
  savedPages: number;
  skippedPages: number;
}

export interface PublishStatus {
  published: number;
  skipped: number;
  decks: string[];
  output: string;
}

export interface TrimStatus {
  output: string;
  slides: number;
  cropped: number;
}

export interface CaptureSettings {
  stabilityFrames: number;
  animationTimeoutMs: number;
  changeThreshold: number;
  blackThreshold: number;
  filterDuplicates: boolean;
}

/** Typed wrapper around the N-API module. Every JSON string is validated here. */
export class SessionHandle {
  private session: native.OhosSession;

  constructor(root: string, filename: string, ratio: string, fit: string, settings: CaptureSettings) {
    this.session = new native.OhosSession(root, filename, ratio, fit, JSON.stringify(settings));
  }

  initError(): string {
    return this.session.initError();
  }

  pushFrame(data: Uint8Array, width: number, height: number, timestampMs: number): FrameStatus {
    return JSON.parse(this.session.pushFrame(data, width, height, timestampMs)) as FrameStatus;
  }

  finish(): PublishStatus {
    return JSON.parse(this.session.finish()) as PublishStatus;
  }

  state(): string {
    return this.session.state();
  }

  sessionDirectory(): string {
    return this.session.sessionDirectory();
  }
}

export function recoverSession(directory: string, settings: CaptureSettings): PublishStatus {
  return JSON.parse(native.recoverSession(directory, JSON.stringify(settings))) as PublishStatus;
}

export function trimDeck(source: string, outputDir: string, placement: string): TrimStatus {
  return JSON.parse(native.trimDeck(source, outputDir, placement)) as TrimStatus;
}
