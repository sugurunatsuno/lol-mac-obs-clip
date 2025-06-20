// エントリーポイント用のJSファイル
// 通知テストボタンのクリックで Rust 側コマンドを呼び出すサンプル
const { invoke } = window.__TAURI__.core;

window.addEventListener("DOMContentLoaded", () => {
  const btn = document.getElementById("notifyTestBtn");
  if (btn) {
    btn.addEventListener("click", () => {
      invoke("show_notification", {
        title: "テスト通知",
        body: "JS から呼び出しました",
      });
    });
  }
}); // DOM 準備完了後に実行
