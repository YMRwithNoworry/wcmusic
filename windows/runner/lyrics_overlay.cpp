#include "lyrics_overlay.h"

#include <algorithm>
#include <cmath>
#include <gdiplus.h>
#include <windowsx.h>

#pragma comment(lib, "gdiplus.lib")

namespace {
constexpr wchar_t kWindowClassName[] = L"WCMusicLyricsOverlay";
constexpr int kOverlayWidth = 480;
constexpr int kDragHandleWidth = 18;
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
  const int work_height = work_area.bottom - work_area.top;
  const int width = std::min(kOverlayWidth, std::max(380, work_width / 3));
  const int height = work_height;
  int x = style_.has_position
      ? style_.x
      : work_area.right - width;
  int y = style_.has_position
      ? style_.y
      : work_area.top;
  window_ = CreateWindowEx(
      WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE |
          0,
      kWindowClassName, L"WCMusic Desktop Lyrics", WS_POPUP, x, y, width,
      height, nullptr, nullptr, instance, this);
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
                           int current_index, float line_progress) {
  const int next_index = lines.empty()
      ? 0
      : std::clamp(current_index, 0, static_cast<int>(lines.size()) - 1);
  const bool index_changed = next_index != current_index_;
  lines_ = lines;
  line_progress_ = std::clamp(line_progress, 0.0f, 1.0f);
  if (lines_.empty()) {
    if (window_) InvalidateRect(window_, nullptr, TRUE);
    return;
  }
  if (index_changed) {
    if (animation_t_ < 1.0f) {
      displayed_index_ = static_cast<int>(
          std::round(Lerp(static_cast<float>(displayed_index_),
                          static_cast<float>(current_index_),
                          EaseInOutCubic(animation_t_))));
    } else {
      displayed_index_ = current_index_;
    }
    current_index_ = next_index;
    animation_t_ = 0.0f;
  }
  if (window_) {
    if (index_changed) SetTimer(window_, kLyricsTimerId, 16, nullptr);
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
  } else {
    RECT work_area{};
    SystemParametersInfo(SPI_GETWORKAREA, 0, &work_area, 0);
    const int work_width = work_area.right - work_area.left;
    const int width = std::min(kOverlayWidth, std::max(380, work_width / 3));
    SetWindowPos(window_, HWND_TOPMOST, work_area.right - width, work_area.top,
                 width, work_area.bottom - work_area.top, SWP_NOACTIVATE);
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
  SetWindowLongPtr(window_, GWL_EXSTYLE, ex_style & ~WS_EX_TRANSPARENT);
  SetWindowPos(window_, nullptr, 0, 0, 0, 0,
               SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE |
                   SWP_FRAMECHANGED);
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
    case WM_NCHITTEST: {
      if (!overlay) return HTTRANSPARENT;
      POINT point{GET_X_LPARAM(lparam), GET_Y_LPARAM(lparam)};
      ScreenToClient(window, &point);
      if (!overlay->style_.locked || point.x <= kDragHandleWidth) {
        return HTCAPTION;
      }
      return HTTRANSPARENT;
    }
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

    const float handle_height = 96.0f;
    Gdiplus::RectF handle_rect(0.0f, (height - handle_height) / 2.0f, 9.0f,
                              handle_height);
    Gdiplus::GraphicsPath handle_path;
    AddRoundedRectangle(handle_path, handle_rect, 4.5f);
    Gdiplus::SolidBrush handle_brush(Gdiplus::Color(150, 92, 94, 98));
    graphics.FillPath(&handle_brush, &handle_path);

    DrawLyricsWheel(graphics, width, height);
  }

  RECT window_bounds{};
  GetWindowRect(window_, &window_bounds);
  POINT destination{window_bounds.left, window_bounds.top};
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
  const float center_y = height * 0.62f;
  const float spacing = std::max(42.0f, style_.font_size * 1.85f);
  const float max_distance = std::ceil(height / spacing / 2.0f) + 1.0f;
  const float left_padding = 34.0f;
  const float right_padding = 20.0f;

  Gdiplus::FontFamily font_family(style_.font_family.c_str());
  const Gdiplus::Color text_color(
      255, GetRValue(style_.text_color), GetGValue(style_.text_color),
      GetBValue(style_.text_color));

  for (int index = 0; index < static_cast<int>(lines_.size()); ++index) {
    const float offset = static_cast<float>(index) - base;
    const float distance = std::abs(offset);
    if (distance > max_distance) continue;
    const float y = center_y + offset * spacing;
    if (y < -spacing || y > height + spacing) continue;
    const bool is_center = distance < 0.5f;
    const bool is_played = index < current_index_;
    const int font_size = is_center ? style_.font_size + 4 : style_.font_size;
    const float line_width = width - left_padding - right_padding;
    const float line_height = spacing;
    const float edge_fade = std::clamp(
        std::min(y / (spacing * 1.5f),
                 (height - y) / (spacing * 1.5f)),
        0.18f, 1.0f);
    const BYTE alpha = static_cast<BYTE>(
        std::clamp((is_center ? 255.0f : (is_played ? 230.0f : 185.0f)) *
                       edge_fade,
                   0.0f, 255.0f));

    Gdiplus::Font font(&font_family, static_cast<float>(font_size),
                       is_center ? Gdiplus::FontStyleBold
                                 : Gdiplus::FontStyleRegular,
                       Gdiplus::UnitPixel);
    const Gdiplus::Color base_color(
        alpha, text_color.GetRed(), text_color.GetGreen(), text_color.GetBlue());
    const Gdiplus::Color accent_color(alpha, 0, 198, 91);
    Gdiplus::SolidBrush base_brush(is_played ? accent_color : base_color);
    Gdiplus::StringFormat format;
    format.SetFormatFlags(Gdiplus::StringFormatFlagsNoWrap);
    format.SetTrimming(Gdiplus::StringTrimmingEllipsisCharacter);
    format.SetAlignment(
        style_.align == DT_LEFT
            ? Gdiplus::StringAlignmentNear
            : style_.align == DT_RIGHT ? Gdiplus::StringAlignmentFar
                                       : Gdiplus::StringAlignmentCenter);
    format.SetLineAlignment(Gdiplus::StringAlignmentCenter);

    Gdiplus::RectF text_rect(left_padding, y - line_height / 2.0f,
                             line_width, line_height);
    graphics.DrawString(lines_[index].c_str(), -1, &font, text_rect, &format,
                        &base_brush);

    if (is_center && line_progress_ > 0.0f) {
      const Gdiplus::GraphicsState state = graphics.Save();
      Gdiplus::RectF measured;
      graphics.MeasureString(lines_[index].c_str(), -1, &font, text_rect,
                             &format, &measured);
      Gdiplus::RectF clip_rect(measured.X, y - line_height / 2.0f,
                              measured.Width * line_progress_, line_height);
      graphics.SetClip(clip_rect);
      Gdiplus::SolidBrush accent_brush(accent_color);
      graphics.DrawString(lines_[index].c_str(), -1, &font, text_rect, &format,
                          &accent_brush);
      graphics.Restore(state);
    }
  }
}
