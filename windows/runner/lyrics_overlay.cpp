#include "lyrics_overlay.h"

#include <algorithm>
#include <cmath>
#include <gdiplus.h>

#pragma comment(lib, "gdiplus.lib")

namespace {
constexpr wchar_t kWindowClassName[] = L"WCMusicLyricsOverlay";
constexpr int kOverlayHeight = 230;
constexpr UINT_PTR kLyricsTimerId = 0x4C59;
constexpr float kLyricsAnimationSeconds = 0.38f;

float EaseInOutCubic(float value) {
  return value < 0.5f
      ? 4.0f * value * value * value
      : static_cast<float>(1 - std::pow(-2 * value + 2, 3) / 2);
}

float Lerp(float from, float to, float value) {
  return from + (to - from) * value;
}

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

void LyricsOverlay::Update(const std::vector<std::wstring>& lines,
                           int current_index) {
  lines_ = lines;
  current_index_ = current_index < 0 ? 0 : current_index;
  if (lines_.empty()) {
    if (window_) InvalidateRect(window_, nullptr, TRUE);
    return;
  }
  if (animation_t_ < 1.0f) {
    displayed_index_ = static_cast<int>(
        std::round(Lerp(static_cast<float>(displayed_index_),
                        static_cast<float>(current_index_),
                        EaseInOutCubic(animation_t_))));
  } else {
    displayed_index_ = current_index_;
  }
  animation_t_ = 0.0f;
  if (window_) {
    SetTimer(window_, kLyricsTimerId, 16, nullptr);
    InvalidateRect(window_, nullptr, TRUE);
  }
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
    case WM_TIMER:
      if (overlay && wparam == kLyricsTimerId) {
        overlay->animation_t_ += 0.016f / kLyricsAnimationSeconds;
        if (overlay->animation_t_ >= 1.0f) {
          overlay->animation_t_ = 1.0f;
          overlay->displayed_index_ = overlay->current_index_;
          KillTimer(window, kLyricsTimerId);
        }
        InvalidateRect(window, nullptr, TRUE);
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

    DrawLyricsWheel(graphics, width, height);
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

void LyricsOverlay::DrawLyricsWheel(Gdiplus::Graphics& graphics, int width,
                                    int height) {
  if (lines_.empty()) return;
  const float base = animation_t_ < 1.0f
      ? Lerp(static_cast<float>(displayed_index_),
             static_cast<float>(current_index_),
             EaseInOutCubic(animation_t_))
      : static_cast<float>(current_index_);
  const float center_x = width / 2.0f;
  const float center_y = height * 0.46f;
  const float spacing = std::max(60.0f, width * 0.075f);
  const float arc_depth = height * 0.20f;
  const float max_distance = 4.0f;

  Gdiplus::FontFamily font_family(style_.font_family.c_str());
  const Gdiplus::Color text_color(
      255, GetRValue(style_.text_color), GetGValue(style_.text_color),
      GetBValue(style_.text_color));

  for (int index = 0; index < static_cast<int>(lines_.size()); ++index) {
    const float offset = static_cast<float>(index) - base;
    const float distance = std::abs(offset);
    if (distance > max_distance) continue;
    const float clamped = std::min(distance, max_distance);
    const float angle = offset * 0.30f;
    const float x = center_x + std::sin(angle) * spacing * 3.6f;
    const float y = center_y - (1.0f - std::cos(angle)) * arc_depth;
    const float scale = 1.0f - clamped * 0.14f;
    const float alpha = (1.0f - clamped * 0.22f) * 255.0f;
    const int font_size = distance < 0.5f
        ? style_.font_size + 8
        : style_.font_size - 4;
    const bool is_center = distance < 0.5f;
    const float line_width = is_center ? width - 220.0f : width - 360.0f;
    const float line_height = is_center ? 52.0f : 36.0f;

    Gdiplus::Font font(&font_family, static_cast<float>(font_size),
                       is_center ? Gdiplus::FontStyleBold
                                 : Gdiplus::FontStyleRegular,
                       Gdiplus::UnitPixel);
    Gdiplus::SolidBrush brush(
        Gdiplus::Color(static_cast<BYTE>(std::max(0.0f, std::min(alpha, 255.0f))),
                       text_color.GetRed(), text_color.GetGreen(),
                       text_color.GetBlue()));
    Gdiplus::StringFormat format;
    format.SetFormatFlags(Gdiplus::StringFormatFlagsNoWrap);
    format.SetTrimming(Gdiplus::StringTrimmingEllipsisCharacter);
    format.SetAlignment(Gdiplus::StringAlignmentCenter);
    format.SetLineAlignment(Gdiplus::StringAlignmentCenter);

    Gdiplus::RectF text_rect(-line_width / 2, -line_height / 2, line_width,
                             line_height);
    const Gdiplus::GraphicsState state = graphics.Save();
    graphics.TranslateTransform(x, y);
    graphics.RotateTransform(offset * 4.0f);
    graphics.ScaleTransform(scale, scale);
    graphics.DrawString(lines_[index].c_str(), -1, &font, text_rect, &format,
                        &brush);
    graphics.Restore(state);
  }
}
