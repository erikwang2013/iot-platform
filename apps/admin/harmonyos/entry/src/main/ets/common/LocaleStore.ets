import { preferences } from '@kit.ArkData';

const PREF_NAME = 'locale_store';
const KEY_FOLLOW = 'follow_system';
const KEY_LOCALE = 'locale';

export class LocaleStore {
  private static prefs: preferences.Preferences | null = null;

  private static async prefsInstance(): Promise<preferences.Preferences> {
    if (!LocaleStore.prefs) {
      LocaleStore.prefs = await preferences.getPreferences(getContext(), PREF_NAME);
    }
    return LocaleStore.prefs;
  }

  static async getFollowSystem(): Promise<boolean> {
    const p = await LocaleStore.prefsInstance();
    return await p.get(KEY_FOLLOW, true) as boolean;
  }

  static async getLocale(): Promise<string> {
    const p = await LocaleStore.prefsInstance();
    return await p.get(KEY_LOCALE, 'zh') as string;
  }

  static async setLocale(code: string): Promise<void> {
    const p = await LocaleStore.prefsInstance();
    await p.put(KEY_LOCALE, code);
    await p.put(KEY_FOLLOW, false);
    await p.flush();
  }

  static async resetSystem(): Promise<void> {
    const p = await LocaleStore.prefsInstance();
    await p.put(KEY_FOLLOW, true);
    await p.delete(KEY_LOCALE);
    await p.flush();
  }
}
