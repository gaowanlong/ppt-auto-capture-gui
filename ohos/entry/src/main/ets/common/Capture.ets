import screenshot from '@ohos.screenshot';
import { image } from '@kit.ImageKit';
import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

export type FrameHandler = (data: Uint8Array, width: number, height: number) => Promise<void>;

/**
 * Samples the whole screen at the configured interval.
 *
 * The app window is minimized while sampling so the recorder does not capture
 * its own UI. `screenshot.capture()` needs CUSTOM_SCREEN_CAPTURE on PC/2in1,
 * and the system consent dialog is handled by Permissions.ets.
 */
export class ScreenSampler {
  private timer: number = -1;
  private busy: boolean = false;
  private windowStage: window.Window | null = null;
  private onFrame: FrameHandler | null = null;

  async start(context: common.UIAbilityContext, intervalMs: number, handler: FrameHandler): Promise<void> {
    this.onFrame = handler;
    try {
      this.windowStage = await window.getLastWindow(context);
      await this.windowStage.minimize();
    } catch (error) {
      // Minimizing is a convenience, not a requirement: the user can do it manually.
      this.windowStage = null;
    }
    this.timer = setInterval(() => {
      void this.tick();
    }, intervalMs);
  }

  private async tick(): Promise<void> {
    if (this.busy || this.onFrame === null) {
      return;
    }
    this.busy = true;
    let pixelMap: image.PixelMap | null = null;
    try {
      // displayId 0 = primary screen. The desktop build uses one screen too.
      pixelMap = await screenshot.capture({ displayId: 0, rotation: image.ImageRotation.ROTATION_0 });
      const width: number = pixelMap.getImageInfo().size.width;
      const height: number = pixelMap.getImageInfo().size.height;
      const buffer: ArrayBuffer = new ArrayBuffer(pixelMap.getPixelBytesNumber());
      await pixelMap.readPixelsToBuffer(buffer);
      // HarmonyOS PixelMap is BGRA_8888; Frame in the shared Rust core expects BGRA.
      const data = new Uint8Array(buffer);
      await this.onFrame(data, width, height);
    } catch (error) {
      // A failed sample must not kill the recording; the activity log keeps it.
      console.error(`screen sample failed: ${JSON.stringify(error)}`);
    } finally {
      if (pixelMap !== null) {
        pixelMap.release();
      }
      this.busy = false;
    }
  }

  async stop(): Promise<void> {
    if (this.timer >= 0) {
      clearInterval(this.timer);
      this.timer = -1;
    }
    this.onFrame = null;
    if (this.windowStage !== null) {
      try {
        await this.windowStage.restore();
      } catch (error) {
        console.error(`window restore failed: ${JSON.stringify(error)}`);
      }
      this.windowStage = null;
    }
  }
}
