#include "flutter_window.h"

#include <optional>

#include <flutter/standard_method_codec.h>

#include "flutter/generated_plugin_registrant.h"
#include "lyrics_overlay.h"

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  lyrics_overlay_ = std::make_unique<LyricsOverlay>();
  lyrics_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(),
          "wcmusic/lyrics_overlay",
          &flutter::StandardMethodCodec::GetInstance());
  lyrics_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) {
        const auto* arguments = std::get_if<flutter::EncodableMap>(
            call.arguments());
        if (call.method_name() == "setEnabled") {
          bool enabled = false;
          if (arguments) {
            const auto value = arguments->find(
                flutter::EncodableValue("enabled"));
            if (value != arguments->end()) {
              if (const auto flag = std::get_if<bool>(&value->second)) {
                enabled = *flag;
              }
            }
          }
          if (enabled) {
            lyrics_overlay_->Show();
          } else {
            lyrics_overlay_->Hide();
          }
          result->Success(flutter::EncodableValue(true));
          return;
        }
        if (call.method_name() == "update") {
          std::string current_line;
          std::string next_line;
          if (arguments) {
            const auto current = arguments->find(
                flutter::EncodableValue("currentLine"));
            const auto next = arguments->find(
                flutter::EncodableValue("nextLine"));
            if (current != arguments->end()) {
              if (const auto text =
                      std::get_if<std::string>(&current->second)) {
                current_line = *text;
              }
            }
            if (next != arguments->end()) {
              if (const auto text = std::get_if<std::string>(&next->second)) {
                next_line = *text;
              }
            }
          }
          lyrics_overlay_->Update(current_line, next_line);
          result->Success();
          return;
        }
        result->NotImplemented();
      });
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  lyrics_channel_.reset();
  lyrics_overlay_.reset();
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
