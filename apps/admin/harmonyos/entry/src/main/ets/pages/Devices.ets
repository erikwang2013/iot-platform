import { router } from '@kit.ArkUI';

interface DeviceRow {
  name: string;
  vendor: string;
  online: boolean;
}

// ponytail: mock 列表演示 UI 骨架；后端 /api/devices 目前是网关 stub，
// 登录端点落地后换 @kit.NetworkKit 拉真实数据 + JWT 鉴权
const MOCK_DEVICES: DeviceRow[] = [
  { name: '网关-01', vendor: 'tuya', online: true },
  { name: '温湿度计-A', vendor: 'mqtt', online: true },
  { name: '插座-B', vendor: 'tuya', online: false },
  { name: '摄像头-C', vendor: 'mqtt', online: false },
];

@Entry
@Component
struct Devices {
  build() {
    Column() {
      Row() {
        Text($r('app.string.device_my_devices'))
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
        Blank()
        Button($r('app.string.common_back'))
          .fontSize(14)
          .height(32)
          .onClick(() => { router.back(); })
      }
      .width('100%')
      .padding({ left: 16, right: 16, top: 12, bottom: 8 })

      List({ space: 8 }) {
        ForEach(MOCK_DEVICES, (d: DeviceRow) => {
          ListItem() {
            Row({ space: 12 }) {
              Text(d.online ? '●' : '○')
                .fontSize(18)
                .fontColor(d.online ? '#4CAF50' : '#9E9E9E')
              Column({ space: 4 }) {
                Text(d.name).fontSize(16)
                Text(d.vendor + ' · ' + (d.online
                  ? $r('app.string.device_online').toString()
                  : $r('app.string.device_offline').toString()))
                  .fontSize(12)
                  .fontColor('#888888')
              }
              .alignItems(HorizontalAlign.Start)
              Blank()
              Button($r('app.string.device_detail'))
                .fontSize(12)
                .height(30)
                .backgroundColor('#FFFFFF')
                .fontColor($r('app.color.app_primary'))
            }
            .width('100%')
            .padding(12)
            .backgroundColor('#FFFFFF')
            .borderRadius(8)
          }
        }, (d: DeviceRow) => d.name)
      }
      .width('100%')
      .layoutWeight(1)
      .padding({ left: 16, right: 16 })
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#F5F5F5')
  }
}
