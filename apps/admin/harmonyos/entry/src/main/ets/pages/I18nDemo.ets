import { intl, i18n } from '@kit.LocalizationKit';
import { router } from '@kit.ArkUI';
import { LocaleStore } from '../common/LocaleStore';

@Entry
@Component
struct I18nDemo {
  @State dateText: string = '';
  @State numberText: string = '';
  @State currencyText: string = '';
  @State textDirection: string = 'ltr';

  aboutToAppear(): void {
    this.render();
  }

  async render(): Promise<void> {
    const follow = await LocaleStore.getFollowSystem();
    const locale = follow
      ? i18n.System.getSystemLanguage()
      : await LocaleStore.getLocale();
    const now = new Date();
    this.dateText = new intl.DateTimeFormat(locale, { dateStyle: 'full' }).format(now);
    this.numberText = new intl.NumberFormat(locale,
      { style: 'decimal', maximumFractionDigits: 2 }).format(1234567.89);
    this.currencyText = new intl.NumberFormat(locale,
      { style: 'currency', currency: 'CNY' }).format(1234567.89);
    this.textDirection = i18n.isRTL(locale) ? 'rtl' : 'ltr';
  }

  build() {
    Column({ space: 12 }) {
      Row() {
        Text($r('app.string.i18n_demo_title'))
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

      Column({ space: 8 }) {
        Text($r('app.string.i18n_demo_date')).fontSize(14).fontColor('#999999')
        Text(this.dateText).fontSize(16)
      }
      .alignItems(HorizontalAlign.Start)
      .width('100%')

      Column({ space: 8 }) {
        Text($r('app.string.i18n_demo_number')).fontSize(14).fontColor('#999999')
        Text(this.numberText).fontSize(16)
      }
      .alignItems(HorizontalAlign.Start)
      .width('100%')

      Column({ space: 8 }) {
        Text($r('app.string.i18n_demo_currency')).fontSize(14).fontColor('#999999')
        Text(this.currencyText).fontSize(16)
      }
      .alignItems(HorizontalAlign.Start)
      .width('100%')

      Column({ space: 8 }) {
        Text($r('app.string.i18n_demo_direction')).fontSize(14).fontColor('#999999')
        Text(this.textDirection).fontSize(16)
      }
      .alignItems(HorizontalAlign.Start)
      .width('100%')

      Text($r('app.string.i18n_demo_long_text'))
        .fontSize(14)
        .lineHeight(22)
        .width('100%')
        .padding(12)
        .backgroundColor('#FFFFFF')
        .borderRadius(8)
    }
    .width('100%')
    .height('100%')
    .padding({ left: 16, right: 16, top: 8 })
    .backgroundColor('#F5F5F5')
  }
}
