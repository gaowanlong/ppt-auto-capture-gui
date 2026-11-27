import { abilityAccessCtrl, bundleManager, common } from '@kit.AbilityKit';

const CAPTURE_PERMISSION: string = 'ohos.permission.CUSTOM_SCREEN_CAPTURE';

/**
 * Silent check. Must never be used to *request* permission: this call has no
 * foreground requirement and does not show a dialog.
 */
export function hasCapturePermission(): boolean {
  try {
    const info = bundleManager.getBundleInfoForSelfSync(
      bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_APPLICATION
    );
    const status = abilityAccessCtrl.createAtManager().checkAccessTokenSync(
      info.appInfo.accessTokenId,
      CAPTURE_PERMISSION
    );
    return status === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED;
  } catch (error) {
    return false;
  }
}

/** Call only from a foreground button click; the consent dialog attaches to the window. */
export async function requestCapturePermission(context: common.UIAbilityContext): Promise<boolean> {
  try {
    const result = await abilityAccessCtrl.createAtManager().requestPermissionsFromUser(
      context,
      [CAPTURE_PERMISSION]
    );
    return result.authResults.every((item: number) => item === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED);
  } catch (error) {
    return false;
  }
}
