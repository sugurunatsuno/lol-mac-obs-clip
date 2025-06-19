// Windows リリース時に余計なコンソールを表示させないための属性
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // ライブラリ側のエントリポイントを呼び出すだけ
    lol_clip_tool_lib::run() // 実際の処理は lib 側に実装
}
