import { Channel } from '@tauri-apps/api/core';
import { commands, type AppInfo, type InstallOutcome, type UpdateFault, type UpdatePrefs_Serialize, type UpdateProgress_Serialize, type UpdateStatus_Serialize } from './bindings';

export type UpdatePrefs = UpdatePrefs_Serialize;
export type UpdateProgress = UpdateProgress_Serialize;
export type UpdateStatus = UpdateStatus_Serialize;
export type { AppInfo, InstallOutcome, UpdateFault };

export class UpdateError extends Error {
  constructor(readonly fault: UpdateFault) {
    super(fault.code);
  }
}

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UpdateFault };

async function unwrap<T>(r: Promise<Result<T>>): Promise<T> {
  const res = await r;
  if (res.status === 'error') throw new UpdateError(res.error);
  return res.data;
}

export const aboutReal = {
  appInfo: (): Promise<AppInfo> => commands.appInfo(),
  openLink: async (url: string): Promise<void> => {
    await unwrap(commands.openLink(url));
  },
  updateCheck: (auto: boolean): Promise<UpdateStatus> => unwrap(commands.updateCheck(auto)),
  updateDownload: async (onProgress: (p: UpdateProgress) => void): Promise<void> => {
    const channel = new Channel<UpdateProgress>();
    channel.onmessage = onProgress;
    await unwrap(commands.updateDownload(channel as never));
  },
  updateInstall: (): Promise<InstallOutcome> => unwrap(commands.updateInstall()),
  updateRestart: async (): Promise<void> => {
    await unwrap(commands.updateRestart());
  },
  seclabVersionsUpdate: (): Promise<string> => unwrap(commands.seclabVersionsUpdate()),
  updatePrefsGet: (): Promise<UpdatePrefs> => commands.updatePrefsGet(),
  updatePrefsSet: (checkOnStart: boolean): Promise<UpdatePrefs> => commands.updatePrefsSet(checkOnStart),
};
