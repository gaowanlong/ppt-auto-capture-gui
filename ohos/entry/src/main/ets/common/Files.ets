import { common } from '@kit.AbilityKit';
import { fileShare, fs, picker } from '@kit.CoreFileKit';

/** Where journals, screenshots and generated decks live inside the app sandbox. */
export function sessionRoot(context: common.UIAbilityContext): string {
  return `${context.filesDir}/sessions`;
}

/** Document exports live in the sandbox first, then are copied through a URI. */
export function exportRoot(context: common.UIAbilityContext): string {
  return `${context.filesDir}/exports`;
}

export function ensureDirectory(path: string): void {
  try {
    fs.mkdirSync(path);
  } catch (error) {
    // Already exists is the normal case.
  }
}

/** Ask the user for an output folder and keep access across restarts. */
export async function chooseOutputFolder(context: common.UIAbilityContext): Promise<string> {
  const documentPicker = new picker.DocumentViewPicker(context);
  const options = new picker.DocumentSelectOptions();
  options.selectMode = picker.DocumentSelectMode.FOLDER;
  options.maxSelectNumber = 1;
  const uris: string[] = await documentPicker.select(options);
  if (uris.length === 0) {
    throw new Error('No folder selected');
  }
  try {
    await fileShare.persistPermission([uris[0]]);
  } catch (error) {
    // Picker access still works for this session; persistence is best-effort.
    console.error(`persistPermission failed: ${JSON.stringify(error)}`);
  }
  return uris[0];
}

/** Copy a sandbox file to a Picker-granted URI, so the user can find the PPTX. */
export function copyToUri(sourcePath: string, targetUri: string): void {
  const input = fs.openSync(sourcePath, fs.OpenMode.READ_ONLY);
  const bytes = new ArrayBuffer(1024 * 1024);
  const output = fs.openSync(targetUri, fs.OpenMode.WRITE_ONLY | fs.OpenMode.CREATE | fs.OpenMode.TRUNC);
  try {
    let read: number = fs.readSync(input.fd, bytes);
    while (read > 0) {
      fs.writeSync(output.fd, bytes, 0, read);
      read = fs.readSync(input.fd, bytes);
    }
  } finally {
    fs.closeSync(input);
    fs.closeSync(output);
  }
}

/** Ask for the final file URI; default directory comes from the chosen folder. */
export async function saveAsUri(
  context: common.UIAbilityContext,
  folderUri: string,
  filename: string
): Promise<string> {
  const documentPicker = new picker.DocumentViewPicker(context);
  const options = new picker.DocumentSaveOptions();
  options.defaultFilePathUri = folderUri;
  options.newFileNames = [filename];
  options.fileSuffixChoices = ['.pptx'];
  const uris: string[] = await documentPicker.save(options);
  if (uris.length === 0) {
    throw new Error('No save target selected');
  }
  return uris[0];
}

/** Settings live in the sandbox so a UI restart keeps every choice. */
export function readSettings(context: common.UIAbilityContext): string {
  const path: string = `${context.filesDir}/settings.json`;
  try {
    return fs.readTextSync(path);
  } catch (error) {
    return '';
  }
}

export function writeSettings(context: common.UIAbilityContext, json: string): void {
  const path: string = `${context.filesDir}/settings.json`;
  try {
    fs.writeTextSync(path, json);
  } catch (error) {
    hilogWarning(`settings save failed: ${JSON.stringify(error)}`);
  }
}

function hilogWarning(message: string): void {
  console.error(message);
}
