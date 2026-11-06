#include "flutter_window.h"

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <utility>
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

bool ParseHotkey(const std::string& binding, UINT* modifiers, UINT* key) {
  std::vector<std::string> parts;
  std::string current;
  for (const char value : binding) {
    if (value == '-') {
      if (!current.empty()) parts.push_back(current);
      current.clear();
    } else {
      current.push_back(static_cast<char>(std::tolower(
          static_cast<unsigned char>(value))));
    }
  }
  if (!current.empty()) parts.push_back(current);
  UINT flags = 0;
  std::string key_name;
  for (const auto& part : parts) {
    if (part == "ctrl" || part == "control") flags |= MOD_CONTROL;
    else if (part == "alt") flags |= MOD_ALT;
    else if (part == "shift") flags |= MOD_SHIFT;
    else if (part == "win") flags |= MOD_WIN;
    else if (key_name.empty()) key_name = part;
    else return false;
  }
  if (key_name.empty()) return false;
  UINT virtual_key = 0;
  if (key_name.size() == 1 && std::isalnum(
          static_cast<unsigned char>(key_name[0]))) {
    virtual_key = static_cast<UINT>(std::toupper(
        static_cast<unsigned char>(key_name[0])));
  } else if (key_name.size() >= 2 && key_name[0] == 'f') {
    int number = 0;
    for (std::size_t index = 1; index < key_name.size(); ++index) {
      const unsigned char digit = static_cast<unsigned char>(key_name[index]);
      if (!std::isdigit(digit)) return false;
      number = number * 10 + (digit - '0');
      if (number > 24) return false;
    }
    if (number < 1) return false;
    virtual_key = static_cast<UINT>(VK_F1 + number - 1);
  } else if (key_name == "space") virtual_key = VK_SPACE;
  else if (key_name == "enter" || key_name == "return") virtual_key = VK_RETURN;
  else if (key_name == "escape" || key_name == "esc") virtual_key = VK_ESCAPE;
  else if (key_name == "tab") virtual_key = VK_TAB;
  else if (key_name == "left") virtual_key = VK_LEFT;
  else if (key_name == "right") virtual_key = VK_RIGHT;
  else if (key_name == "up") virtual_key = VK_UP;
  else if (key_name == "down") virtual_key = VK_DOWN;
  else if (key_name == "home") virtual_key = VK_HOME;
  else if (key_name == "end") virtual_key = VK_END;
  else if (key_name == "pageup") virtual_key = VK_PRIOR;
  else if (key_name == "pagedown") virtual_key = VK_NEXT;
  else return false;
  if (flags == 0 && (virtual_key < VK_F1 || virtual_key > VK_F24)) return false;
  *modifiers = flags;
  *key = virtual_key;
  return true;
}

constexpr int kFirstHotkeyId = 200;
const std::vector<std::string>& HotkeyActions() {
  static const std::vector<std::string> actions = {
      "previous", "next", "togglePlay", "volumeUp", "volumeDown",
      "mute", "seekForward", "seekBackward", "toggleLyrics", "toggleWindow"};
  return actions;
}

bool RegisterHotkeySet(HWND window,
                       const std::map<std::string, std::string>& bindings,
                       std::map<int, std::string>* registered,
                       std::string* error) {
  std::map<uint64_t, std::string> seen;
  for (const auto& action : HotkeyActions()) {
    const auto entry = bindings.find(action);
    if (entry == bindings.end() || entry->second.empty()) continue;
    UINT modifiers = 0;
    UINT key = 0;
    if (!ParseHotkey(entry->second, &modifiers, &key)) {
      *error = "快捷键格式无效：" + action;
      break;
    }
    const uint64_t chord = (static_cast<uint64_t>(modifiers) << 32) | key;
    if (seen.find(chord) != seen.end()) {
      *error = "快捷键重复：" + action;
      break;
    }
    seen[chord] = action;
    const int id = kFirstHotkeyId + static_cast<int>(
        std::find(HotkeyActions().begin(), HotkeyActions().end(), action) -
        HotkeyActions().begin());
    if (!RegisterHotKey(window, id, modifiers | MOD_NOREPEAT, key)) {
      *error = "快捷键已被其他程序占用：" + action;
      break;
    }
    (*registered)[id] = action;
  }
  if (error->empty()) return true;
  for (const auto& entry : *registered) UnregisterHotKey(window, entry.first);
  registered->clear();
  return false;
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
  hotkeys_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(),
          "wcmusic/global_hotkeys",
          &flutter::StandardMethodCodec::GetInstance());
  hotkeys_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) {
        if (call.method_name() != "setBindings") {
          result->NotImplemented();
          return;
        }
        const auto* arguments = std::get_if<flutter::EncodableMap>(
            call.arguments());
        const auto* bindings_value = FindArgument(arguments, "bindings");
        const auto* bindings_map = bindings_value
            ? std::get_if<flutter::EncodableMap>(bindings_value)
            : nullptr;
        std::map<std::string, std::string> bindings;
        if (bindings_map) {
          for (const auto& entry : *bindings_map) {
            const auto* action = std::get_if<std::string>(&entry.first);
            const auto* binding = std::get_if<std::string>(&entry.second);
            if (action && binding) bindings[*action] = *binding;
          }
        }
        std::string error;
        if (!SetHotkeyBindings(bindings, &error)) {
          result->Error("hotkey_registration_failed", error);
          return;
        }
        result->Success();
      });
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
          if (lines.empty()) {
            const auto current_line = FindArgument(arguments, "currentLine");
            const std::string text = StringValue(current_line, "");
            if (!text.empty()) lines.push_back(Utf8ToWide(text));
          }
          int current_index = static_cast<int>(DoubleValue(
              FindArgument(arguments, "currentIndex"), 0));
          float line_progress = static_cast<float>(DoubleValue(
              FindArgument(arguments, "lineProgress"), 0));
          std::wstring translation = Utf8ToWide(StringValue(
              FindArgument(arguments, "currentTranslation"), ""));
          if (current_index < 0) {
            lines.clear();
            const std::string current_line = StringValue(
                FindArgument(arguments, "currentLine"), "");
            if (!current_line.empty()) lines.push_back(Utf8ToWide(current_line));
            current_index = 0;
            line_progress = 0.0f;
            translation.clear();
          }
          lyrics_overlay_->Update(lines, current_index, line_progress,
                                  translation);
          result->Success();
          return;
        }
        if (call.method_name() == "setStyle") {
          LyricsOverlayStyle style;
          const auto* font = FindArgument(arguments, "fontFamily");
          if (font) style.font_family = Utf8ToWide(StringValue(font, "Microsoft YaHei"));
          style.font_size = static_cast<int>(DoubleValue(
              FindArgument(arguments, "fontSize"), 24));
          const std::string align = StringValue(
              FindArgument(arguments, "align"), "center");
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
              FindArgument(arguments, "cornerRadius"), 8));
          style.locked = BoolValue(FindArgument(arguments, "locked"), false);
          style.always_on_top = BoolValue(
              FindArgument(arguments, "alwaysOnTop"), true);
          style.show_translation = BoolValue(
              FindArgument(arguments, "showTranslation"), true);
          style.zoom_active_line = BoolValue(
              FindArgument(arguments, "zoomActiveLine"), true);
          style.window_width = static_cast<int>(DoubleValue(
              FindArgument(arguments, "windowWidth"), 1000));
          style.window_height = static_cast<int>(DoubleValue(
              FindArgument(arguments, "windowHeight"), 140));
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
  lyrics_overlay_->SetPositionCallback(
      [this](int x, int y, int width, int height) {
        if (!lyrics_channel_) return;
        flutter::EncodableMap map;
        map[flutter::EncodableValue("x")] = flutter::EncodableValue(x);
        map[flutter::EncodableValue("y")] = flutter::EncodableValue(y);
        map[flutter::EncodableValue("width")] = flutter::EncodableValue(width);
        map[flutter::EncodableValue("height")] = flutter::EncodableValue(height);
        lyrics_channel_->InvokeMethod(
            "positionChanged",
            std::make_unique<flutter::EncodableValue>(std::move(map)));
      });
  lyrics_overlay_->SetActionCallback([this](const std::string& action) {
    if (!lyrics_channel_) return;
    flutter::EncodableMap map;
    map[flutter::EncodableValue("action")] = flutter::EncodableValue(action);
    lyrics_channel_->InvokeMethod(
        "toolbarAction",
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
  hotkeys_channel_.reset();
  lyrics_channel_.reset();
  for (const auto& entry : registered_hotkeys_) {
    UnregisterHotKey(GetHandle(), entry.first);
  }
  registered_hotkeys_.clear();
  lyrics_overlay_.reset();
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

bool FlutterWindow::SetHotkeyBindings(
    const std::map<std::string, std::string>& bindings, std::string* error) {
  const auto previous = hotkey_values_;
  for (const auto& entry : registered_hotkeys_) {
    UnregisterHotKey(GetHandle(), entry.first);
  }
  registered_hotkeys_.clear();
  error->clear();
  if (RegisterHotkeySet(GetHandle(), bindings, &registered_hotkeys_, error)) {
    hotkey_values_ = bindings;
    return true;
  }
  std::string rollback_error;
  RegisterHotkeySet(GetHandle(), previous, &registered_hotkeys_,
                    &rollback_error);
  return false;
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  if (message == WM_HOTKEY) {
    const auto found = registered_hotkeys_.find(static_cast<int>(wparam));
    if (found != registered_hotkeys_.end() && hotkeys_channel_) {
      hotkeys_channel_->InvokeMethod(
          "action",
          std::make_unique<flutter::EncodableValue>(found->second));
    }
    return 0;
  }

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
