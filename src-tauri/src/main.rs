// 릴리스 빌드에서 콘솔 창이 함께 뜨지 않게 한다. 지우지 말 것.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    services_dashboard_lib::run();
}
