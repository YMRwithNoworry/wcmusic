#include "lyrics_overlay.h"

#include <algorithm>

namespace {
constexpr wchar_t kWindowClassName[] = L"WCMusicLyricsOverlay";
constexpr int kOverlayHeight = 104;
}

LyricsOverlay::LyricsOverlay() = default;

LyricsOverlay::~LyricsOverlay() {
  Destroy();
}

bool LyricsOverlay::Create() {
  if (window_) return true;
  const HINSTANCE instance = GetModuleHandle(nullptr);
  WNDCLASS window_class{};
  window_class.lpfnWndProc = LyricsOverlay::WindowProc;
  window_class.hInstance = instance;
  window_class.lpszClassName = kWindowClassName;
  window_class.hCursor = LoadCursor(nullptr, IDC_ARROW);
  RegisterClass(&window_class);

  RECT work_area{};
  SystemParametersInfo(SPI_GETWORKAREA, 0, &work_area, 0);
  const int work_width = work_area.right - work_area.left;
  const int width = std::min(900, std::max(420, work_width - 80));
  int x = style_.has_position
      ? style_.x
      : work_area.left + (work_width - width) / 2;
  int y = style_.has_position
      ? style_.y
      : work_area.bottom - kOverlayHeight - 72;
  window_ = CreateWindowEx(
      WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE |
          WS_EX_TRANSPARENT,
      kWindowClassName, L"WCMusic Desktop Lyrics", WS_POPUP, x, y, width,
      kOverlayHeight, nullptr, nullptr, instance, this);
  if (!window_) return false;
  ApplyWindowAttributes();
  return true;
}

void LyricsOverlay::Show() {
  if (!Create()) return;
  ShowWindow(window_, SW_SHOWNOACTIVATE);
  SetWindowPos(window_, HWND_TOPMOST, 0, 0, 0, 0,
               SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
}

void LyricsOverlay::Hide() {
  if (window_) ShowWindow(window_, SW_HIDE);
}

void LyricsOverlay::Update(const std::string& current_line,
                           const std::string& next_line) {
  current_line_ = Utf8ToWide(current_line);
  next_line_ = Utf8ToWide(next_line);
  if (window_) InvalidateRect(window_, nullptr, TRUE);
}

void LyricsOverlay::ApplyStyle(const LyricsOverlayStyle& style) {
  style_ = style;
  if (!window_) return;
  ApplyWindowAttributes();
  if (style_.has_position) {
    SetWindowPos(window_, HWND_TOPMOST, style_.x, style_.y, 0, 0,
                 SWP_NOSIZE | SWP_NOACTIVATE);
  }
  InvalidateRect(window_, nullptr, TRUE);
}

void LyricsOverlay::SetPositionCallback(PositionCallback callback) {
  position_callback_ = std::move(callback);
}

void LyricsOverlay::Destroy() {
  if (!window_) return;
  DestroyWindow(window_);
  window_ = nullptr;
}

void LyricsOverlay::ApplyWindowAttributes() {
  SetLayeredWindowAttributes(
      window_, 0, static_cast<BYTE>(style_.opacity), LWA_ALPHA);
  RECT bounds{};
  GetClientRect(window_, &bounds);
  const int radius = std::max(0, style_.corner_radius);
  SetWindowRgn(
      window_,
      CreateRoundRectRgn(0, 0, bounds.right, bounds.bottom, radius, radius),
      TRUE);
  const LONG_PTR ex_style = GetWindowLongPtr(window_, GWL_EXSTYLE);
  if (style_.locked) {
    SetWindowLongPtr(window_, GWL_EXSTYLE, ex_style | WS_EX_TRANSPARENT);
  } else {
    SetWindowLongPtr(window_, GWL_EXSTYLE, ex_style & ~WS_EX_TRANSPARENT);
  }
}

LRESULT CALLBACK LyricsOverlay::WindowProc(HWND window, UINT message,
                                           WPARAM wparam, LPARAM lparam) {
  LyricsOverlay* overlay = reinterpret_cast<LyricsOverlay*>(
      GetWindowLongPtr(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    const auto create = reinterpret_cast<CREATESTRUCT*>(lparam);
    overlay = static_cast<LyricsOverlay*>(create->lpCreateParams);
    SetWindowLongPtr(window, GWLP_USERDATA,
                     reinterpret_cast<LONG_PTR>(overlay));
  }
  switch (message) {
    case WM_ERASEBKGND:
      return 1;
    case WM_PAINT:
      if (overlay) overlay->Paint();
      return 0;
    case WM_NCHITTEST:
      if (overlay && !overlay->style_.locked) return HTCAPTION;
      return HTTRANSPARENT;
    case WM_EXITSIZEMOVE:
      if (overlay && !overlay->style_.locked && overlay->position_callback_) {
        RECT bounds{};
        GetWindowRect(window, &bounds);
        overlay->position_callback_(bounds.left, bounds.top);
      }
      return 0;
    case WM_DESTROY:
      return 0;
  }
  return DefWindowProc(window, message, wparam, lparam);
}

void LyricsOverlay::Paint() {
  PAINTSTRUCT paint{};
  HDC dc = BeginPaint(window_, &paint);
  RECT bounds{};
  GetClientRect(window_, &bounds);
  HBRUSH background = CreateSolidBrush(style_.background_color);
  FillRect(dc, &bounds, background);
  DeleteObject(background);
  SetBkMode(dc, TRANSPARENT);

  const int align_flag = style_.align == DT_LEFT
      ? DT_LEFT
      : style_.align == DT_RIGHT ? DT_RIGHT : DT_CENTER;

  RECT current_bounds = bounds;
  current_bounds.left += 28;
  current_bounds.right -= 28;
  current_bounds.top = 14;
  current_bounds.bottom = 64;
  HFONT current_font = CreateFont(
      -style_.font_size, 0, 0, 0, FW_SEMIBOLD, FALSE, FALSE, FALSE,
      DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
      CLEARTYPE_QUALITY, DEFAULT_PITCH | FF_DONTCARE,
      style_.font_family.c_str());
  HFONT previous_font =
      static_cast<HFONT>(SelectObject(dc, current_font));
  SetTextColor(dc, style_.text_color);
  DrawText(dc, current_line_.c_str(), -1, &current_bounds,
           align_flag | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);

  RECT next_bounds = bounds;
  next_bounds.left += 32;
  next_bounds.right -= 32;
  next_bounds.top = 62;
  next_bounds.bottom -= 10;
  const int next_size = std::max(12, style_.font_size * 3 / 5);
  HFONT next_font = CreateFont(
      -next_size, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET,
      OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
      DEFAULT_PITCH | FF_DONTCARE, style_.font_family.c_str());
  SelectObject(dc, next_font);
  SetTextColor(dc, style_.text_color);
  DrawText(dc, next_line_.c_str(), -1, &next_bounds,
           align_flag | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);

  SelectObject(dc, previous_font);
  DeleteObject(current_font);
  DeleteObject(next_font);
  EndPaint(window_, &paint);
}

std::wstring LyricsOverlay::Utf8ToWide(const std::string& value) {
  if (value.empty()) return L"";
  const int size = MultiByteToWideChar(CP_UTF8, 0, value.data(),
                                       static_cast<int>(value.size()), nullptr, 0);
  std::wstring result(size, L'\0');
  MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()),
                      result.data(), size);
  return result;
}
