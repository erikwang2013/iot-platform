import { i18n } from '@kit.LocalizationKit';
import { router } from '@kit.ArkUI';
import { LocaleStore } from '../common/LocaleStore';

const CODES: string[] = ['zh', 'en', 'ko', 'ru', 'de', 'fr', 'es', 'pt', 'hi', 'ar', 'bn', 'id', 'ja'];

@Entry
@Component
struct Settings {
  @State followSystem: boolean = true;
  @State current: string = 'zh';

  aboutToAppear(): void {
    this.refresh();
  }

  async refresh(): Promise<void> {
    this.followSystem = await LocaleStore.getFollowSystem();
    this.current = this.followSystem
      ? i18n.System.getSystemLanguage()
      : await LocaleStore.getLocale();
  }

  async select(code: string): Promise<void> {
    i18n.System.setAppPreferredLanguage(code);
    await LocaleStore.setLocale(code);
    this.followSystem = false;
    this.current = code;
  }

  async follow(): Promise<void> {
    await LocaleStore.resetSystem();
    this.followSystem = true;
    this.current = i18n.System.getSystemLanguage();
  }

  build() {
    Column() {
      Row() {
        Text($r('app.string.settings_language'))
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
        Blank()
        Button($r('app.string.common_back'))
          .fontSize(14)
          .height(32)
          .onClick(() => {
            router.back();
          })
      }
      .width('100%')
      .padding({ left: 16, right: 16, top: 12, bottom: 8 })

      Text($r('app.string.settings_language_list_title'))
        .fontSize(14)
        .fontColor('#999999')
        .width('100%')
        .padding({ left: 16 })

      List({ space: 4 }) {
        ListItem() {
          Row() {
            Text($r('app.string.settings_language_system'))
              .fontSize(16)
            Blank()
            if (this.followSystem) {
              Text($r('app.string.common_ok'))
                .fontColor($r('app.color.app_primary'))
            }
          }
          .width('100%')
          .padding(12)
          .onClick(() => {
            this.follow();
          })
        }
        ForEach(CODES, (code: string) => {
          ListItem() {
            Row() {
              Text(i18n.getDisplayLanguage(code, code, true))
                .fontSize(16)
              Blank()
              if (!this.followSystem && this.current === code) {
                Text($r('app.string.common_ok'))
                  .fontColor($r('app.color.app_primary'))
              }
            }
            .width('100%')
            .padding(12)
            .onClick(() => {
              this.select(code);
            })
          }
        }, (code: string) => code)
      }
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#F5F5F5')
  }
}
