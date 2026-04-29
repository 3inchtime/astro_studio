<p align="center">
  <img src="../src/assets/logo.png" alt="Astro Studio" width="360" />
</p>

<h1 align="center">Astro Studio</h1>

<p align="center">
  自由接入第三方圖片生成 API 的桌面平台
  <br />
  A desktop platform for connecting the image generation APIs you choose.
</p>

<p align="center">
  <a href="../README.md">English</a> · <a href="./README.zh-CN.md">简体中文</a> · <strong>繁體中文</strong> · <a href="./README.ja.md">日本語</a> · <a href="./README.ko.md">한국어</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey" alt="Platform" />
  <img src="https://img.shields.io/badge/Product-AI%20Image%20Studio-5b7cff" alt="Product" />
  <img src="https://img.shields.io/badge/License-MIT-green" alt="License" />
</p>

---

Astro Studio 是一個圍繞 AI 圖像創作打造的桌面平台，重點不是綁定某一家模型服務，而是讓你可以更自由地接入第三方圖片生成 API，用同一套介面管理不同服務、不同模型和持續發生的創作過程。

它想解決的不是「如何完成一次生成」，而是「如何把圖像生成真正變成穩定、順手、長期可用的工作流」。

## 產品定位

Astro Studio 不是單一模型的演示殼，也不是只強調一次性出圖的網頁工具。

它更像一個屬於創作者自己的圖像生成工作台：

- 你可以自由配置第三方圖片生成 API
- 你可以把不同供應商接入同一個桌面入口
- 你可以在同一個空間裡完成生成、回看、篩選、收藏和繼續編輯
- 你可以把創作歷史沉澱為長期可管理的素材資產

如果你不希望自己的工作流被某一個平台綁定，Astro Studio 就是為這種使用方式而設計的。

## 適配進度

- [x] `gpt-image-2`
- [ ] `nano banana` - 適配測試中
- [ ] `nano banana 2` - 適配測試中
- [ ] `nano banana pro` - 適配測試中
- [ ] 更多第三方圖片生成 API

## 為什麼做這個產品

現在很多 AI 圖片產品都在強調模型能力本身，卻很少認真處理「創作之後」的問題：

- 歷史記錄很快變亂
- 喜歡的結果難以整理
- 想繼續迭代時，上下文已經斷掉
- 更換服務商時，整個使用習慣也要跟著重來

Astro Studio 希望把這些零散環節重新組織起來，讓圖像生成從一次次試驗，變成一條連續的創作流程。

## 核心價值

### 自由接入

Astro Studio 的核心不是預設某個固定平台，而是支持自由配置 API Key、Base URL 與模型入口，讓你可以按自己的方式接入第三方圖片生成能力。

### 統一入口

無論你使用官方服務、代理閘道，還是兼容介面的第三方提供方，都可以盡量在同一個桌面介面裡完成操作，而不是在多個網頁後台之間反覆切換。

### 面向長期創作

它關注的不只是「生成成功」，還包括歷史積累、作品回看、收藏整理、繼續編輯與後續迭代，讓創作結果不是生成完就消失。

### 桌面體驗

相比網頁工具，Astro Studio 更強調穩定、專注、持續使用的感覺。它更像一個每天都會打開的創作工作台，而不是偶爾使用一次的線上頁面。

## 適合誰

- 經常使用 AI 生成圖片的設計師、插畫師、內容創作者
- 需要同時使用不同圖片生成服務的個人用戶或小團隊
- 希望自由切換 API 供應商、不願被單一平台綁定的人
- 想把歷史、收藏、素材和配置長期沉澱在自己桌面環境裡的用戶

## 你可以用它做什麼

- 輸入提示詞，快速發起圖片生成
- 接入你自己的第三方圖片生成 API
- 在不同服務與模型之間建立統一的使用入口
- 瀏覽完整歷史，回看每一次創作結果
- 收藏喜歡的圖片，並按資料夾整理
- 基於已有圖片繼續編輯和迭代
- 透過搜尋更快找到過去生成過的內容
- 用更穩定的本地桌面方式管理整個創作過程

## 當前體驗重點

- 自訂 API 接入
- 圖片生成與繼續編輯
- 歷史會話沉澱
- 畫廊瀏覽
- 收藏夾整理
- 回收站恢復
- 本地化創作管理

## 我們相信什麼

Astro Studio 相信，未來的圖像創作工具不應該只是某個模型的附屬介面，而應該是一個更中立、更開放、更長期可用的平台。

模型會變化，供應商會變化，介面也會變化，但創作者對工作流的掌控不應該跟著一起丟失。

所以 Astro Studio 想做的，是把「選擇權」重新交還給使用者：

- 選擇接入誰
- 選擇怎麼用
- 選擇如何保留自己的創作歷史
- 選擇怎樣組織自己的素材世界

## 取得方式

前往 [Releases](https://github.com/3inchtime/astro_studio/releases) 下載對應平台的安裝套件。

首次啟動後，只需要完成自己的 API 配置，就可以開始使用 Astro Studio 作為第三方圖片生成服務的統一桌面入口。

## 本地構建

如果你想從原始碼本地執行 Astro Studio，先準備好基礎環境：

- Node.js `22+`
- npm `11+`
- Rust stable toolchain
- 當前作業系統所需的 Tauri 依賴

安裝專案依賴：

```bash
npm install
```

以桌面應用開發模式啟動：

```bash
npm run tauri dev
```

如果你只想單獨啟動前端頁面：

```bash
npm run dev
```

執行生產構建：

```bash
npm run build
npm run tauri build
```

當前 GitHub 自動發布流程只產出 Windows 安裝套件。如果你在本地構建 macOS 版本自用，應用通常可以在自己的機器上執行；但如果要公開發行，仍然需要額外完成 Apple 簽名與公證。

## Roadmap

- [ ] 接入更多第三方圖片生成服務
- [ ] 支持更完整的多供應商切換體驗
- [ ] 提供更強的專案化創作管理能力
- [ ] 增強圖片編輯與參考圖工作流
- [ ] 繼續完善收藏、篩選與素材組織體驗

## 授權條款

本專案採用 [MIT License](LICENSE)。
