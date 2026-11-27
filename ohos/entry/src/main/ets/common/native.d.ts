declare module 'libppt_capture_ohos_core.so' {
  export class OhosSession {
    constructor(root: string, filename: string, ratio: string, fit: string, configJson: string);
    initError(): string;
    pushFrame(data: Uint8Array, width: number, height: number, timestampMs: number): string;
    finish(): string;
    state(): string;
    sessionDirectory(): string;
  }
  export function openSession(directory: string, configJson: string): OhosSession;
  export function recoverSession(directory: string, configJson: string): string;
  export function trimDeck(source: string, outputDir: string, placement: string): string;
}
