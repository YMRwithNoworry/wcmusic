#include "lyrics_overlay.h"

#include <algorithm>
#include <gdiplus.h>

#pragma comment(lib, "gdiplus.lib")

namespace {
constexpr wchar_t kWindowClassName[] = L"WCMusicLyricsOverlay";
constexpr int kOverlayHeight = 104;

void AddRoundedRectangle(Gdiplus::GraphicsPath& path,
                         const Gdiplus::RectF& rect, float radius) {
  if (radius <= 0) {
    path.AddRectangle(rect);
    return;
  }
  const float diameter = radius * 2;
  path.AddArc(rect.X, rect.Y, diameter, diameter, 180, 90);
  path.AddArc(rect.X + rect.Width - diameter, rect.Y, diameter, diameter, 270,
              90);
  path.AddArc(rect.X + rect.Width - diameter,
              rect.Y + rect.Height - diameter, diameter, diameter, 0, 90);
  path.AddArc(rect.X, rect.Y + rect.Height - diameter, diameter, diameter, 90,
              90);
  path.CloseFigure();
}
}

LyricsOverlay::LyricsOverlay() = default;

LyricsOverlay::~LyricsOverlay() {
  Destroy();
}

bool LyricsOverlay::Create() {
  if (window_) return true;
  static ULONG_PTR gdiplus_token = 0;
  if (gdiplus_token == 0) {
    Gdiplus::GdiplusStartupInput input;
    Gdiplus::GdiplusStartup(&gdiplus_token, &input, nullptr);
  }
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
  RECT bounds{};
  GetClientRect(window_, &bounds);
  const int width = bounds.right - bounds.left;
  const int height = bounds.bottom - bounds.top;
  if (width <= 0 || height <= 0) return;

  HDC screen_dc = GetDC(nullptr);
  HDC mem_dc = CreateCompatibleDC(screen_dc);
  BITMAPINFO bitmap_info{};
  bitmap_info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
  bitmap_info.bmiHeader.biWidth = width;
  bitmap_info.bmiHeader.biHeight = -height;
  bitmap_info.bmiHeader.biPlanes = 1;
  bitmap_info.bmiHeader.biBitCount = 32;
  bitmap_info.bmiHeader.biCompression = BI_RGB;
  void* bits = nullptr;
  HBITMAP dib = CreateDIBSection(
      screen_dc, &bitmap_info, DIB_RGB_COLORS, &bits, nullptr, 0);
  HGDIOBJ previous = SelectObject(mem_dc, dib);

  {
    Gdiplus::Graphics graphics(mem_dc);
    graphics.Clear(Gdiplus::Color(0, 0, 0, 0));
    graphics.SetTextRenderingHint(Gdiplus::TextRenderingHintAntiAlias);

    Gdiplus::RectF full(0, 0, static_cast<float>(width),
                        static_cast<float>(height));
    Gdiplus::GraphicsPath background_path;
    AddRoundedRectangle(
        background_path, full, static_cast<float>(style_.corner_radius));
    Gdiplus::Color background(
        static_cast<BYTE>(style_.opacity),
        GetRValue(style_.background_color),
        GetGValue(style_.background_color),
        GetBValue(style_.background_color));
    Gdiplus::SolidBrush background_brush(background);
    graphics.FillPath(&background_brush, &background_path);

    Gdiplus::FontFamily font_family(style_.font_family.c_str());
    Gdiplus::StringFormat format;
    format.SetFormatFlags(Gdiplus::StringFormatFlagsNoWrap);
    format.SetLineAlignment(Gdiplus::StringAlignmentCenter);
    if (style_.align == DT_LEFT) {
      format.SetAlignment(Gdiplus::StringAlignmentNear);
    } else if (style_.align == DT_RIGHT) {
      format.SetAlignment(Gdiplus::StringAlignmentFar);
    } else {
      format.SetAlignment(Gdiplus::StringAlignmentCenter);
    }

    Gdiplus::Color text_color(
        255, GetRValue(style_.text_color), GetGValue(style_.text_color),
        GetBValue(style_.text_color));
    Gdiplus::SolidBrush text_brush(text_color);

    Gdiplus::Font current_font(
        &font_family, static_cast<float>(style_.font_size),
        Gdiplus::FontStyleBold, Gdiplus::UnitPixel);
    Gdiplus::RectF current_rect(28, 8, static_cast<float>(width - 56), 50);
    graphics.DrawString(current_line_.c_str(), -1, &current_font,
                        current_rect, &format, &text_brush);

    const int next_size = std::max(12, style_.font_size * 3 / 5);
    Gdiplus::Font next_font(
        &font_family, static_cast<float>(next_size),
        Gdiplus::FontStyleRegular, Gdiplus::UnitPixel);
    Gdiplus::RectF next_rect(32, 56, static_cast<float>(width - 64),
                             static_cast<float>(height - 66));
    graphics.DrawString(next_line_.c_str(), -1, &next_font, next_rect,
                        &format, &text_brush);
  }

  POINT destination{0, 0};
  POINT source{0, 0};
  SIZE size{width, height};
  BLENDFUNCTION blend{};
  blend.BlendOp = AC_SRC_OVER;
  blend.SourceConstantAlpha = 255;
  blend.AlphaFormat = AC_SRC_ALPHA;
  UpdateLayeredWindow(window_, screen_dc, &destination, &size, mem_dc,
                      &source, 0, &blend, ULW_ALPHA);

  SelectObject(mem_dc, previous);
  DeleteObject(dib);
  DeleteDC(mem_dc);
  ReleaseDC(nullptr, screen_dc);
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
