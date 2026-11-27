import { common } from '@kit.AbilityKit';
import { picker } from '@kit.CoreFileKit';
import { ScreenSampler } from '../common/Capture';
import { chooseOutputFolder, copyToUri, ensureDirectory, exportRoot, readSettings, saveAsUri, sessionRoot, writeSettings } from '../common/Files';
import { CaptureSettings, FrameStatus, PublishStatus, SessionHandle, TrimStatus, trimDeck } from '../common/NativeBridge';
import { hasCapturePermission, requestCapturePermission } from '../common/Permissions';

const NAV_RECORD: string = 'record';
const NAV_SETTINGS: string = 'settings';
const NAV_APPEARANCE: string = 'appearance';
const NAV_TRIM: string = 'trim';

interface ActivityEntry {
  level: string;
  message: string;
}

@Component
struct NavItem {
  label: string = '';
  selected: boolean = false;
  onClickAction: () => void = () => {};

  build() {
    Row() {
      Text(this.label)
        .fontSize(15)
        .fontWeight(this.selected ? FontWeight.Medium : FontWeight.Normal)
        .fontColor(this.selected ? '#0B57D0' : '#3C4043')
        .layoutWeight(1)
    }
    .padding({ left: 16, right: 16, top: 12, bottom: 12 })
    .backgroundColor(this.selected ? '#E8F0FE' : Color.Transparent)
    .borderRadius(10)
    .onClick(this.onClickAction)
  }
}

@Entry
@Component
struct Index {
  @State private tab: string = NAV_RECORD;
  @State private dark: boolean = false;
  @State private permGranted: boolean = hasCapturePermission();
  @State private recording: boolean = false;
  @State private settings: CaptureSettings = {
    stabilityFrames: 2,
    animationTimeoutMs: 10000,
    changeThreshold: 0.05,
    blackThreshold: 0.80,
    filterDuplicates: true
  };
  @State private sampleIntervalMs: number = 200;
  @State private outputFolderUri: string = '';
  @State private status: string = 'Ready';
  @State private savedPages: number = 0;
  @State private skippedPages: number = 0;
  @State private activity: ActivityEntry[] = [];

  private context: common.UIAbilityContext | null = AppStorage.get('appContext') as common.UIAbilityContext;
  private sampler: ScreenSampler = new ScreenSampler();
  private session: SessionHandle | null = null;
  private outputFilename: string = `ppt-capture-${Date.now()}.pptx`;

  aboutToAppear(): void {
    this.restoreSettings();
    this.permGranted = hasCapturePermission();
    this.log('info', 'Application started');
  }

  private restoreSettings(): void {
    const json: string = readSettings(this.context);
    if (json.length === 0) {
      return;
    }
    try {
      const parsed = JSON.parse(json) as Record<string, Object>;
      this.sampleIntervalMs = Number(parsed['sampleIntervalMs'] ?? 200);
      const nested = parsed['settings'] as CaptureSettings;
      if (nested !== undefined) {
        this.settings = nested;
      }
      this.outputFolderUri = String(parsed['outputFolderUri'] ?? '');
    } catch (error) {
      this.log('warn', 'Saved settings could not be read');
    }
  }

  private persistSettings(): void {
    if (this.context === null) {
      return;
    }
    writeSettings(this.context, JSON.stringify({
      sampleIntervalMs: this.sampleIntervalMs,
      settings: this.settings,
      outputFolderUri: this.outputFolderUri,
      dark: this.dark
    }));
  }

  private log(level: string, message: string): void {
    const entry: ActivityEntry = { level: level, message: message };
    this.activity = [entry, ...this.activity].slice(0, 60);
    this.status = message;
  }

  private async enablePermission(): Promise<void> {
    if (this.context === null) {
      return;
    }
    const granted: boolean = await requestCapturePermission(this.context);
    this.permGranted = granted;
    this.log(granted ? 'info' : 'warn', granted ? 'Screen capture permission granted' : 'Screen capture permission denied');
  }

  private async chooseFolder(): Promise<void> {
    if (this.context === null) {
      return;
    }
    try {
      this.outputFolderUri = await chooseOutputFolder(this.context);
      this.persistSettings();
      this.log('info', 'Output folder selected and saved');
    } catch (error) {
      this.log('warn', `Output folder selection failed: ${JSON.stringify(error)}`);
    }
  }

  private async startRecording(): Promise<void> {
    if (this.context === null) {
      return;
    }
    if (!this.permGranted) {
      this.log('warn', 'Grant screen capture permission first');
      return;
    }
    const root: string = sessionRoot(this.context);
    ensureDirectory(root);
    ensureDirectory(exportRoot(this.context));
    this.outputFilename = `ppt-capture-${Date.now()}.pptx`;
    const handle = new SessionHandle(root, this.outputFilename, '16:9', 'fit', this.settings);
    const error: string = handle.initError();
    if (error.length > 0) {
      this.log('warn', `Cannot start recording: ${error}`);
      return;
    }
    this.session = handle;
    this.savedPages = 0;
    this.skippedPages = 0;
    this.recording = true;
    await this.sampler.start(this.context, this.sampleIntervalMs, async (data: Uint8Array, width: number, height: number) => {
      const outcome: FrameStatus = handle.pushFrame(data, width, height, Date.now());
      this.savedPages = outcome.savedPages;
      this.skippedPages = outcome.skippedPages;
      if (outcome.saved) {
        this.log('info', `Saved slide ${outcome.savedPages}`);
      }
    });
    this.log('info', 'Recording started');
  }

  private async stopRecording(): Promise<void> {
    if (this.session === null) {
      return;
    }
    await this.sampler.stop();
    this.recording = false;
    try {
      const report: PublishStatus = this.session.finish();
      this.savedPages = report.published;
      this.skippedPages = report.skipped;
      this.log('info', `PPTX generated: ${report.published} slides`);
      if (this.outputFolderUri.length > 0 && report.decks.length > 0) {
        const targetUri: string = await saveAsUri(this.context, this.outputFolderUri, this.outputFilename);
        copyToUri(report.decks[0], targetUri);
        this.log('info', `Exported to ${targetUri}`);
      }
    } catch (error) {
      this.log('warn', `Stop failed: ${JSON.stringify(error)}`);
    }
    this.session = null;
  }

  private async trimExistingDeck(): Promise<void> {
    this.log('info', 'Choose a PPTX to trim');
    try {
      const pickerResult: string[] = await this.selectPptx();
      if (pickerResult.length === 0) {
        return;
      }
      const result: TrimStatus = trimDeck(pickerResult[0], exportRoot(this.context), 'fill');
      this.log('info', `Trimmed ${result.cropped}/${result.slides} slides: ${result.output}`);
    } catch (error) {
      this.log('warn', `Trim failed: ${JSON.stringify(error)}`);
    }
  }

  private async selectPptx(): Promise<string[]> {
    const documentPicker = new picker.DocumentViewPicker(this.context);
    const options = new picker.DocumentSelectOptions();
    options.maxSelectNumber = 1;
    options.fileSuffixFilters = ['PPTX|.pptx'];
    return await documentPicker.select(options);
  }

  private card(content: () => void): void {
    Column() {
      content();
    }
    .width('100%')
    .padding(16)
    .margin({ bottom: 12 })
    .backgroundColor(this.dark ? '#202124' : Color.White)
    .borderRadius(14)
    .border({ width: 1, color: this.dark ? '#3C4043' : '#E8EAED' })
  }

  private sectionTitle(text: string): void {
    Text(text)
      .fontSize(18)
      .fontWeight(FontWeight.Medium)
      .fontColor(this.dark ? '#E8EAED' : '#202124')
      .margin({ bottom: 12 })
  }

  private recordView(): void {
    this.card(() => {
      this.sectionTitle('Screen capture permission');
      Row() {
        Text(this.permGranted ? 'Granted' : 'Not granted')
          .fontSize(14)
          .fontColor(this.permGranted ? '#188038' : '#C5221F')
          .layoutWeight(1);
        Button(this.permGranted ? 'Refresh' : 'Grant')
          .onClick(() => { void this.enablePermission(); });
      }
      .width('100%');
      Text('Full screen (primary display) is captured. The app window is minimized while recording.')
        .fontSize(12)
        .fontColor(this.dark ? '#9AA0A6' : '#5F6368')
        .margin({ top: 8 });
    });

    this.card(() => {
      this.sectionTitle('Recording');
      Row() {
        Text(`Screenshots: ${this.savedPages}`).fontSize(14).layoutWeight(1);
        Text(`Skipped: ${this.skippedPages}`).fontSize(14).layoutWeight(1);
      }
      .width('100%');
      Row() {
        Button('Start')
          .enabled(!this.recording && this.permGranted)
          .onClick(() => { void this.startRecording(); });
        Button('Stop')
          .enabled(this.recording)
          .onClick(() => { void this.stopRecording(); });
      }
      .width('100%')
      .margin({ top: 12 });
      Text(this.status)
        .fontSize(13)
        .fontColor(this.dark ? '#9AA0A6' : '#5F6368')
        .margin({ top: 10 });
    });

    this.card(() => {
      this.sectionTitle('Activity');
      List() {
        ForEach(this.activity, (entry: ActivityEntry) => {
          ListItem() {
            Text(`${entry.level.toUpperCase()}  ${entry.message}`)
              .fontSize(12)
              .fontColor(entry.level === 'warn' ? '#C5221F' : (this.dark ? '#E8EAED' : '#3C4043'));
          }
        }, (entry: ActivityEntry) => entry.message);
      }
      .height(180);
    });
  }

  private settingsView(): void {
    this.card(() => {
      this.sectionTitle('Capture settings');
      Row() {
        Text('Sample interval (ms)').fontSize(14).layoutWeight(1);
        TextInput({ text: `${this.sampleIntervalMs}` })
          .width(100)
          .type(InputType.Number)
          .onChange((value: string) => {
            this.sampleIntervalMs = Number(value) > 0 ? Number(value) : 200;
            this.persistSettings();
          });
      }
      .width('100%');
      Row() {
        Text('Stable frames').fontSize(14).layoutWeight(1);
        TextInput({ text: `${this.settings.stabilityFrames}` })
          .width(100)
          .type(InputType.Number)
          .onChange((value: string) => {
            this.settings.stabilityFrames = Number(value) > 0 ? Number(value) : 2;
            this.persistSettings();
          });
      }
      .width('100%')
      .margin({ top: 10 });
      Row() {
        Text('Animation timeout (ms)').fontSize(14).layoutWeight(1);
        TextInput({ text: `${this.settings.animationTimeoutMs}` })
          .width(100)
          .type(InputType.Number)
          .onChange((value: string) => {
            this.settings.animationTimeoutMs = Number(value) > 0 ? Number(value) : 10000;
            this.persistSettings();
          });
      }
      .width('100%')
      .margin({ top: 10 });
      Row() {
        Text('Duplicate filtering').fontSize(14).layoutWeight(1);
        Toggle({ type: ToggleType.Switch, isOn: this.settings.filterDuplicates })
          .onChange((isOn: boolean) => {
            this.settings.filterDuplicates = isOn;
            this.persistSettings();
          });
      }
      .width('100%')
      .margin({ top: 10 });
      Text('Transition frames are skipped: only a frame that stayed still for at least one sample becomes a slide.')
        .fontSize(12)
        .fontColor(this.dark ? '#9AA0A6' : '#5F6368')
        .margin({ top: 10 });
    });
  }

  private appearanceView(): void {
    this.card(() => {
      this.sectionTitle('Appearance');
      Row() {
        Text('Dark theme').fontSize(14).layoutWeight(1);
        Toggle({ type: ToggleType.Switch, isOn: this.dark })
          .onChange((isOn: boolean) => {
            this.dark = isOn;
            this.persistSettings();
          });
      }
      .width('100%');
      Text('Card layout and system font follow the desktop build.')
        .fontSize(12)
        .fontColor(this.dark ? '#9AA0A6' : '#5F6368')
        .margin({ top: 10 });
    });
  }

  private trimView(): void {
    this.card(() => {
      this.sectionTitle('Trim an existing PPTX');
      Text('Choose a deck; black borders and stable toolbars are trimmed. Original file is not modified.')
        .fontSize(13)
        .fontColor(this.dark ? '#9AA0A6' : '#5F6368');
      Button('Choose PPTX and trim')
        .margin({ top: 12 })
        .onClick(() => { void this.trimExistingDeck(); });
    });

    this.card(() => {
      this.sectionTitle('Output folder');
      Text(this.outputFolderUri.length > 0 ? this.outputFolderUri : 'No folder selected')
        .fontSize(12)
        .fontColor(this.dark ? '#9AA0A6' : '#5F6368');
      Button('Choose folder')
        .margin({ top: 12 })
        .onClick(() => { void this.chooseFolder(); });
    });
  }

  build() {
    Row() {
      Column() {
        Text('PPT Auto Capture')
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
          .fontColor(this.dark ? '#E8EAED' : '#202124')
          .padding({ left: 16, top: 20, bottom: 20 });
        NavItem({ label: 'Record', selected: this.tab === NAV_RECORD, onClickAction: () => { this.tab = NAV_RECORD; } });
        NavItem({ label: 'Settings', selected: this.tab === NAV_SETTINGS, onClickAction: () => { this.tab = NAV_SETTINGS; } });
        NavItem({ label: 'Appearance', selected: this.tab === NAV_APPEARANCE, onClickAction: () => { this.tab = NAV_APPEARANCE; } });
        NavItem({ label: 'Create PPTX', selected: this.tab === NAV_TRIM, onClickAction: () => { this.tab = NAV_TRIM; } });
      }
      .width(210)
      .height('100%')
      .backgroundColor(this.dark ? '#292A2D' : '#F8F9FA');

      Scroll() {
        Column() {
          if (this.tab === NAV_RECORD) {
            this.recordView();
          } else if (this.tab === NAV_SETTINGS) {
            this.settingsView();
          } else if (this.tab === NAV_APPEARANCE) {
            this.appearanceView();
          } else {
            this.trimView();
          }
        }
        .width('100%')
        .padding(20)
      }
      .layoutWeight(1)
      .height('100%')
      .backgroundColor(this.dark ? '#1F1F1F' : Color.White)
    }
    .width('100%')
    .height('100%')
    .fontFamily('HarmonyOS Sans SC')
  }
}
