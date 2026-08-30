import { router } from '@kit.ArkUI';

// 登录页：用户名/密码 → 后端 /api/auth/login。后端登录端点尚未落地，
// 网关连通后在此接入 @kit.NetworkKit（HTTP + JWT 存储）。
@Entry
@Component
struct Login {
  @State username: string = '';
  @State password: string = '';
  @State error: string = '';

  doLogin(): void {
    if (this.username === '' || this.password === '') {
      this.error = $r('app.string.login_required').toString();
      return;
    }
    // ponytail: 无后端登录端点，先只做表单校验；接入后替换为 httpRequest + token 持久化
    this.error = '登录端点未就绪（P4 网关接入后可用）';
  }

  build() {
    Column({ space: 12 }) {
      Text($r('app.string.login_title'))
        .fontSize(24)
        .fontWeight(FontWeight.Bold)
      Text($r('app.string.login_subtitle'))
        .fontSize(14)
        .fontColor('#888888')
      TextInput({ placeholder: $r('app.string.login_username').toString() })
        .onChange((v: string) => { this.username = v; })
        .backgroundColor('#FFFFFF')
        .borderRadius(8)
      TextInput({ placeholder: $r('app.string.login_password').toString() })
        .type(InputType.Password)
        .onChange((v: string) => { this.password = v; })
        .backgroundColor('#FFFFFF')
        .borderRadius(8)
      if (this.error !== '') {
        Text(this.error)
          .fontSize(14)
          .fontColor('#D32F2F')
      }
      Button($r('app.string.login_btn'))
        .width('100%')
        .onClick(() => { this.doLogin(); })
      Button($r('app.string.common_back'))
        .width('100%')
        .backgroundColor('#FFFFFF')
        .fontColor('#333333')
        .onClick(() => { router.back(); })
    }
    .width('100%')
    .height('100%')
    .padding(24)
    .justifyContent(FlexAlign.Center)
    .backgroundColor('#F5F5F5')
  }
}
