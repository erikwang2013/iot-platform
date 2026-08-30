import { i18n } from '@kit.LocalizationKit';
import { router } from '@kit.ArkUI';
import { LocaleStore } from '../common/LocaleStore';

@Entry
@Component
struct Index {
  @State currentLocale: string = 'zh';

  aboutToAppear(): void {
    this.refreshLocale();
  }

  async refreshLocale(): Promise<void> {
    const follow = await LocaleStore.getFollowSystem();
    this.currentLocale = follow
      ? i18n.System.getSystemLanguage()
      : await LocaleStore.getLocale();
  }

  build() {
    Column({ space: 16 }) {
      Text($r('app.string.app_name'))
        .fontSize(28)
        .fontWeight(FontWeight.Bold)
      Text('当前语言: ' + this.currentLocale + ' '
        + i18n.getDisplayLanguage(this.currentLocale, this.currentLocale, true))
        .fontSize(16)
        .fontColor($r('app.color.app_primary'))
      Button($r('app.string.login_btn'))
        .width(200)
        .onClick(() => {
          router.pushUrl({ url: 'pages/Login' });
        })
      Button($r('app.string.device_my_devices'))
        .width(200)
        .onClick(() => {
          router.pushUrl({ url: 'pages/Devices' });
        })
      Button($r('app.string.nav_settings'))
        .width(200)
        .onClick(() => {
          router.pushUrl({ url: 'pages/Settings' });
        })
      Button($r('app.string.i18n_demo_title'))
        .width(200)
        .onClick(() => {
          router.pushUrl({ url: 'pages/I18nDemo' });
        })
    }
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
  }
}
