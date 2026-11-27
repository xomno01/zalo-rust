# Zalo Ultra Native (Rust + Tauri v2)

Ứng dụng Zalo Desktop độc lập viết bằng **Rust + Tauri v2** — không cần cài đặt Zalo PC chính thức, hỗ trợ cả **Windows** và **macOS** với mức tiêu thụ RAM tối ưu.

An independent, lightweight **Rust + Tauri v2** desktop client for Zalo Web — zero official Zalo PC installation required, supporting **Windows** and **macOS** with isolated multi-account WebView profiles.

## Tính năng chính / Key Features

- **100% Standalone**: Tải trực tiếp `https://chat.zalo.me` bằng WebView gốc của hệ điều hành (`WebView2` trên Windows, `WKWebView` trên macOS).
- **Multi-Account Isolation**: Mỗi tài khoản sở hữu thư mục dữ liệu / session tách biệt hoàn toàn (`data_directory` trên Windows, `data_store_identifier` UUID xác định trên macOS).
- **System Tray & Close-to-Tray**: Ẩn xuống khay hệ thống khi đóng cửa sổ chính; mở nhanh tài khoản mặc định, nén RAM, hoặc thoát hoàn toàn từ menu tray.
- **Win32 RAM Trim**: Giải phóng bộ nhớ nhàn rỗi (`EmptyWorkingSet` qua `windows-sys`) theo yêu cầu hoặc tự động khi thu gọn về khay hệ thống.
- **Cross-Platform CI/CD**: Tự động đóng gói installer cho Windows (`.msi` / `.exe`) và macOS Universal (`.dmg` / `.app`) qua GitHub Actions.

## Yêu cầu hệ thống / Prerequisites

- [Rust & Cargo](https://rustup.rs/) (stable)
- Windows: WebView2 Runtime (có sẵn trên Windows 10/11)
- macOS: Xcode Command Line Tools

## Phát triển & Kiểm thử / Build & Test

```bash
# Chạy unit tests (profiles, memory)
cd src-tauri
cargo test

# Chạy chế độ phát triển (cần tauri-cli: cargo install tauri-cli --version "^2")
cargo tauri dev

# Build bản phát hành tối ưu dung lượng (release)
cargo build --release
# Hoặc tạo installer đầy đủ bằng Tauri CLI:
cargo tauri build
```

## Cấu trúc thư mục / Project Structure

- `src-tauri/` — Backend Rust + Tauri v2 (`profiles.rs`, `memory.rs`, `tray.rs`, `lib.rs`, `main.rs`)
- `ui/` — Giao diện quản lý tài khoản thuần HTML/CSS/JS (không cần bước build Node.js)
- `.github/workflows/release.yml` — GitHub Actions build & release tự động cho Windows và macOS
