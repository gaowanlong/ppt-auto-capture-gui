import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';

export default class EntryAbility extends UIAbility {
  onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    AppStorage.setOrCreate('appContext', this.context);
    hilog.info(0x0000, 'ppt-capture', 'EntryAbility onCreate');
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    windowStage.loadContent('pages/Index', (error) => {
      if (error.code !== 0) {
        hilog.error(0x0000, 'ppt-capture', `loadContent failed: ${JSON.stringify(error)}`);
      }
    });
  }

  onForeground(): void {
    // Permission status is re-read by the UI on resume.
  }

  onWindowStageDestroy(): void {
    hilog.info(0x0000, 'ppt-capture', 'EntryAbility onWindowStageDestroy');
  }

  onDestroy(): void {
    hilog.info(0x0000, 'ppt-capture', 'EntryAbility onDestroy');
  }
}
