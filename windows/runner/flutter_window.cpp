#include "flutter_window.h"

#include <optional>
#include <vector>

#include <flutter/standard_method_codec.h>

#include "flutter/generated_plugin_registrant.h"
#include "lyrics_overlay.h"

namespace {

std::wstring Utf8ToWide(const std::string& value) {
  if (value.empty()) return L"";
  const int size = MultiByteToWideChar(CP_UTF8, 0, value.data(),
                                       static_cast<int>(value.size()), nullptr, 0);
  std::wstring result(size, L'\0');
  MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()),
                      result.data(), size);
  return result;
}

COLORREF ArgbToColorRef(int64_t value) {
  return RGB((value >> 16) & 0xFF, (value >> 8) & 0xFF, value & 0xFF);
}

const flutter::EncodableValue* FindArgument(
    const flutter::EncodableMap* arguments,
    const char* key) {
  if (!arguments) return nullptr;
  const auto it = arguments->find(flutter::EncodableValue(key));
  return it == arguments->end() ? nullptr : &it->second;
}

double DoubleValue(const flutter::EncodableValue* value, double fallback) {
  if (!value || value->IsNull()) return fallback;
  if (const auto number = std::get_if<double>(value)) return *number;
  if (const auto number = std::get_if<int32_t>(value)) {
    return static_cast<double>(*number);
  }
  if (const auto number = std::get_if<int64_t>(value)) {
    return static_cast<double>(*number);
  }
  return fallback;
}

bool BoolValue(const flutter::EncodableValue* value, bool fallback) {
  if (!value || value->IsNull()) return fallback;
  if (const auto flag = std::get_if<bool>(value)) return *flag;
  return fallback;
}

std::string StringValue(const flutter::EncodableValue* value,
                        const std::string& fallback) {
  if (!value || value->IsNull()) return fallback;
  if (const auto text = std::get_if<std::string>(value)) return *text;
  return fallback;
}

}  // namespace

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
          std::vector<std::wstring> lines;
          const auto* lines_value = FindArgument(arguments, "lines");
          if (lines_value) {
            if (const auto* list =
                    std::get_if<std::vector<flutter::EncodableValue>>(
                        lines_value)) {
              for (const auto& item : *list) {
                if (const auto* text = std::get_if<std::string>(&item)) {
                  lines.push_back(Utf8ToWide(*text));
                }
              }
            }
          }
          const int current_index = static_cast<int>(DoubleValue(
              FindArgument(arguments, "currentIndex"), 0));
          const float line_progress = static_cast<float>(DoubleValue(
              FindArgument(arguments, "lineProgress"), 0));
          lyrics_overlay_->Update(lines, current_index, line_progress);
          result->Success();
          return;
        }
        if (call.method_name() == "setStyle") {
          LyricsOverlayStyle style;
          const auto* font = FindArgument(arguments, "fontFamily");
          if (font) style.font_family = Utf8ToWide(StringValue(font, "Microsoft YaHei UI"));
          style.font_size = static_cast<int>(DoubleValue(
              FindArgument(arguments, "fontSize"), 24));
          const std::string align = StringValue(
              FindArgument(arguments, "align"), "right");
          if (align == "left") {
            style.align = DT_LEFT;
          } else if (align == "right") {
            style.align = DT_RIGHT;
          } else {
            style.align = DT_CENTER;
          }
          style.text_color = ArgbToColorRef(static_cast<int64_t>(
              DoubleValue(FindArgument(arguments, "textColor"), 0xFFF3F3F3)));
          style.background_color = ArgbToColorRef(static_cast<int64_t>(
              DoubleValue(FindArgument(arguments, "backgroundColor"),
                          0xFF141416)));
          style.opacity = static_cast<int>(
              DoubleValue(FindArgument(arguments, "opacity"), 0.92) * 255);
          style.corner_radius = static_cast<int>(DoubleValue(
              FindArgument(arguments, "cornerRadius"), 0));
          style.locked = BoolValue(FindArgument(arguments, "locked"), true);
          const auto* x = FindArgument(arguments, "x");
          const auto* y = FindArgument(arguments, "y");
          if (x && y && !x->IsNull() && !y->IsNull()) {
            style.has_position = true;
            style.x = static_cast<int>(DoubleValue(x, 0));
            style.y = static_cast<int>(DoubleValue(y, 0));
          }
          lyrics_overlay_->ApplyStyle(style);
          result->Success();
          return;
        }
        result->NotImplemented();
      });
  lyrics_overlay_->SetPositionCallback([this](int x, int y) {
    if (!lyrics_channel_) return;
    flutter::EncodableMap map;
    map[flutter::EncodableValue("x")] = flutter::EncodableValue(x);
    map[flutter::EncodableValue("y")] = flutter::EncodableValue(y);
    lyrics_channel_->InvokeMethod(
        "positionChanged",
        std::make_unique<flutter::EncodableValue>(std::move(map)));
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
