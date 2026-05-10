// Native OS notifications via tauri-plugin-notification. Wraps the
// permission dance so callers don't have to remember to ask.
//
// Used by:
//   - schedule firings (success / failure)
//   - long-running queue completion
//   - error toasts (optional escalation when the app is in the
//     background)

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { TAURI_AVAILABLE } from "$lib/api";

let _granted = false;
let _checked = false;

async function ensure(): Promise<boolean> {
  if (!TAURI_AVAILABLE) return false;
  if (_checked) return _granted;
  _checked = true;
  try {
    _granted = await isPermissionGranted();
    if (!_granted) {
      const p = await requestPermission();
      _granted = p === "granted";
    }
  } catch {
    _granted = false;
  }
  return _granted;
}

export async function notify(title: string, body: string): Promise<void> {
  if (!(await ensure())) return;
  try {
    sendNotification({ title, body });
  } catch {
    /* swallow — notifications are advisory */
  }
}
