#include "lyrics_overlay.h"

#include <algorithm>
#include <gdiplus.h>
#include <windowsx.h>

#pragma comment(lib, "gdiplus.lib")

namespace {
constexpr wchar_t kWindowClassName[] = L"WCMusicLyricsOverlay";
constexpr int kMinOverlayWidth = 600;
constexpr int kMinOverlayHeight = 100;
constexpr int kDragHandleWidth = 18;
constexpr int kResizeBorder = 16;
constexpr int kToolbarButtonWidth = 48;
constexpr int kToolbarButtonGap = 4;
constexpr int kToolbarTop = 8;
constexpr int kToolbarRight = 10;
constexpr int kToolbarButtonCount = 9;

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
  const int width = std::clamp(
      style_.window_width, std::min(kMinOverlayWidth, work_width), work_width);
  const int height = std::clamp(
      style_.window_height, std::min(kMinOverlayHeight, work_height),
      work_height);
  const int x = style_.has_position
      ? style_.x
      : work_area.left + (work_width - width) / 2;
  const int y = style_.has_position
      ? style_.y
      : work_area.bottom - height - 32;
  window_ = CreateWindowEx(
      WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
      kWindowClassName, L"WCMusic Desktop Lyrics", WS_POPUP | WS_THICKFRAME, x, y, width,
      height, nullptr, nullptr, instance, this);
  if (!window_) return false;
  drag_handle_ = CreateWindowEx(
      WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
      kWindowClassName, L"WCMusic Lyrics Drag Handle", WS_POPUP, x, y, 18, 48,
      nullptr, nullptr, instance, this);
  if (!drag_handle_) {
    DestroyWindow(window_);
    window_ = nullptr;
    return false;
  }
  ApplyWindowAttributes();
  UpdateHandlePosition();
  return true;
}

void LyricsOverlay::Show() {
  if (!Create()) return;
  toolbar_visible_ = !style_.locked;
  ShowWindow(window_, SW_SHOWNOACTIVATE);
  SetWindowPos(window_, style_.always_on_top ? HWND_TOPMOST : HWND_NOTOPMOST,
               0, 0, 0, 0,
               SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
  UpdateHandlePosition();
  InvalidateRect(window_, nullptr, TRUE);
}

void LyricsOverlay::Hide() {
  if (drag_handle_) ShowWindow(drag_handle_, SW_HIDE);
  if (window_) ShowWindow(window_, SW_HIDE);
}

void LyricsOverlay::Update(const std::vector<std::wstring>& lines,
                           int current_index, float line_progress,
                           const std::wstring& translation) {
  lines_ = lines;
  line_progress_ = std::clamp(line_progress, 0.0f, 1.0f);
  current_translation_ = translation;
  current_index_ = lines_.empty()
      ? 0
      : std::clamp(current_index, 0, static_cast<int>(lines_.size()) - 1);
  if (window_) InvalidateRect(window_, nullptr, TRUE);
}

void LyricsOverlay::ApplyStyle(const LyricsOverlayStyle& style) {
  style_ = style;
  style_.window_width = std::clamp(style_.window_width, kMinOverlayWidth, 1800);
  style_.window_height = std::clamp(style_.window_height, kMinOverlayHeight, 360);
  toolbar_visible_ = !style_.locked;
  if (!window_) return;
  ApplyWindowAttributes();
  RECT work_area{};
  SystemParametersInfo(SPI_GETWORKAREA, 0, &work_area, 0);
  const int work_width = work_area.right - work_area.left;
  const int work_height = work_area.bottom - work_area.top;
  const int width = std::clamp(style_.window_width,
                               std::min(kMinOverlayWidth, work_width),
                               work_width);
  const int height = std::clamp(style_.window_height,
                                std::min(kMinOverlayHeight, work_height),
                                work_height);
  const int x = style_.has_position
      ? style_.x
      : work_area.left + (work_width - width) / 2;
  const int y = style_.has_position
      ? style_.y
      : work_area.bottom - height - 32;
  SetWindowPos(window_, style_.always_on_top ? HWND_TOPMOST : HWND_NOTOPMOST,
               x, y, width, height, SWP_NOACTIVATE);
  ApplyWindowAttributes();
  UpdateHandlePosition();
  InvalidateRect(window_, nullptr, TRUE);
}

void LyricsOverlay::SetPositionCallback(PositionCallback callback) {
  position_callback_ = std::move(callback);
}

void LyricsOverlay::SetActionCallback(ActionCallback callback) {
  action_callback_ = std::move(callback);
}

void LyricsOverlay::Destroy() {
  if (drag_handle_) {
    DestroyWindow(drag_handle_);
    drag_handle_ = nullptr;
  }
  if (!window_) return;
  DestroyWindow(window_);
  window_ = nullptr;
}

void LyricsOverlay::ApplyWindowAttributes() {
  RECT bounds{};
  GetWindowRect(window_, &bounds);
  const int width = bounds.right - bounds.left;
  const int height = bounds.bottom - bounds.top;
  const int radius = std::max(0, style_.corner_radius);
  SetWindowRgn(
      window_, CreateRoundRectRgn(0, 0, width, height, radius * 2, radius * 2),
      TRUE);
  LONG_PTR ex_style = GetWindowLongPtr(window_, GWL_EXSTYLE);
  // Keep the drag handle interactive while the layered lyrics window passes clicks through.
  if (style_.locked) {
    ex_style |= WS_EX_TRANSPARENT;
  } else {
    ex_style &= ~static_cast<LONG_PTR>(WS_EX_TRANSPARENT);
  }
  SetWindowLongPtr(window_, GWL_EXSTYLE, ex_style);
  SetWindowPos(window_, nullptr, 0, 0, 0, 0,
               SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE |
                   SWP_FRAMECHANGED);
}

void LyricsOverlay::UpdateHandlePosition() {
  if (!window_ || !drag_handle_) return;
  RECT bounds{};
  GetWindowRect(window_, &bounds);
  const int height = bounds.bottom - bounds.top;
  SetWindowPos(drag_handle_,
               style_.always_on_top ? HWND_TOPMOST : HWND_NOTOPMOST,
               bounds.left, bounds.top + (height - 48) / 2, 18, 48,
               SWP_NOACTIVATE);
  if (style_.locked && IsWindowVisible(window_)) {
    ShowWindow(drag_handle_, SW_SHOWNOACTIVATE);
  } else {
    ShowWindow(drag_handle_, SW_HIDE);
  }
}(HWND window, UINT message,
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
      if (overlay) {
        if (window == overlay->drag_handle_) {
          overlay->PaintHandle();
        } else {
          overlay->Paint();
        }
      }
      return 0;
    case WM_NCHITTEST: {
      if (!overlay) return HTTRANSPARENT;
      if (window == overlay->drag_handle_) return HTCAPTION;
      POINT point{GET_X_LPARAM(lparam), GET_Y_LPARAM(lparam)};
      RECT bounds{};
      GetWindowRect(window, &bounds);
      point.x -= bounds.left;
      point.y -= bounds.top;
      const int width = bounds.right - bounds.left;
      const int height = bounds.bottom - bounds.top;
      if (overlay->style_.locked) {
        return point.x <= kDragHandleWidth && point.y > 12 &&
                       point.y < height - 12
                   ? HTCAPTION
                   : HTTRANSPARENT;
      }
      if (overlay->toolbar_visible_ &&
          overlay->ToolbarButtonAt(point, width) >= 0) {
        return HTCLIENT;
      }
      if (point.x >= width - kResizeBorder &&
          point.y >= height - kResizeBorder) {
        return HTBOTTOMRIGHT;
      }
      return HTCAPTION;
    }
    case WM_MOUSEMOVE:
    case WM_NCMOUSEMOVE:
      if (overlay && !overlay->style_.locked) {
        if (!overlay->toolbar_visible_) {
          overlay->toolbar_visible_ = true;
          InvalidateRect(window, nullptr, TRUE);
        }
        if (!overlay->tracking_mouse_leave_) {
          TRACKMOUSEEVENT tracking{};
          tracking.cbSize = sizeof(tracking);
          tracking.dwFlags = TME_LEAVE;
          if (message == WM_NCMOUSEMOVE) tracking.dwFlags |= TME_NONCLIENT;
          tracking.hwndTrack = window;
          overlay->tracking_mouse_leave_ = TrackMouseEvent(&tracking) != FALSE;
        }
      }
      return 0;
    case WM_MOUSELEAVE:
    case WM_NCMOUSELEAVE:
      if (overlay) {
        overlay->tracking_mouse_leave_ = false;
        POINT cursor{};
        RECT bounds{};
        GetCursorPos(&cursor);
        GetWindowRect(window, &bounds);
        if (!PtInRect(&bounds, cursor) && !overlay->style_.locked &&
            overlay->toolbar_visible_) {
          overlay->toolbar_visible_ = false;
          InvalidateRect(window, nullptr, TRUE);
        }
      }
      return 0;
    case WM_LBUTTONUP: {
      if (!overlay || overlay->style_.locked || !overlay->action_callback_) {
        return 0;
      }
      RECT bounds{};
      GetWindowRect(window, &bounds);
      POINT point{GET_X_LPARAM(lparam), GET_Y_LPARAM(lparam)};
      ClientToScreen(window, &point);
      point.x -= bounds.left;
      point.y -= bounds.top;
      const int button = overlay->ToolbarButtonAt(
          point, bounds.right - bounds.left);
      static const char* actions[kToolbarButtonCount] = {
          "fontDown", "fontUp", "opacityDown", "opacityUp", "toggleZoom",
          "toggleTranslation", "toggleAlwaysOnTop", "toggleLock", "close"};
      if (button >= 0 && button < kToolbarButtonCount) {
        overlay->action_callback_(actions[button]);
      }
      return 0;
    }
    case WM_EXITSIZEMOVE:
      if (overlay && window == overlay->drag_handle_) {
        RECT handle_bounds{};
        RECT overlay_bounds{};
        GetWindowRect(window, &handle_bounds);
        GetWindowRect(overlay->window_, &overlay_bounds);
        const int overlay_height = overlay_bounds.bottom - overlay_bounds.top;
        SetWindowPos(
            overlay->window_,
            overlay->style_.always_on_top ? HWND_TOPMOST : HWND_NOTOPMOST,
            handle_bounds.left,
            handle_bounds.top - (overlay_height - 48) / 2, 0, 0,
            SWP_NOSIZE | SWP_NOACTIVATE);
        GetWindowRect(overlay->window_, &overlay_bounds);
        overlay->style_.has_position = true;
        overlay->style_.x = overlay_bounds.left;
        overlay->style_.y = overlay_bounds.top;
        overlay->UpdateHandlePosition();
        if (overlay->position_callback_) {
          overlay->position_callback_(
              overlay_bounds.left, overlay_bounds.top,
              overlay_bounds.right - overlay_bounds.left, overlay_height);
        }
        return 0;
      }
      if (overlay && overlay->position_callback_) {
        RECT bounds{};
        GetWindowRect(window, &bounds);
        overlay->style_.window_width = bounds.right - bounds.left;
        overlay->style_.window_height = bounds.bottom - bounds.top;
        overlay->style_.has_position = true;
        overlay->style_.x = bounds.left;
        overlay->style_.y = bounds.top;
        overlay->ApplyWindowAttributes();
        overlay->position_callback_(bounds.left, bounds.top,
                                   bounds.right - bounds.left,
                                   bounds.bottom - bounds.top);
      }
      return 0;
    case WM_GETMINMAXINFO:
      if (overlay && window == overlay->window_ &&
          !overlay->style_.locked) {
        auto* limits = reinterpret_cast<MINMAXINFO*>(lparam);
        limits->ptMinTrackSize.x = kMinOverlayWidth;
        limits->ptMinTrackSize.y = kMinOverlayHeight;
      }
      return 0;
    case WM_SIZE:
      if (overlay) {
        if (window == overlay->drag_handle_) {
          InvalidateRect(window, nullptr, TRUE);
          return 0;
        }
        overlay->ApplyWindowAttributes();
        overlay->UpdateHandlePosition();
        InvalidateRect(window, nullptr, TRUE);
      }
      return 0;
    case WM_MOUSEACTIVATE:
      return MA_NOACTIVATE;
    case WM_DESTROY:
      return 0;
  }
  return DefWindowProc(window, message, wparam, lparam);
}

void LyricsOverlay::Paint() {
  RECT bounds{};
  GetWindowRect(window_, &bounds);
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
    if (toolbar_visible_ && !style_.locked) DrawToolbar(graphics, width);
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

void LyricsOverlay::PaintHandle() {
  if (!drag_handle_) return;
  constexpr int width = 18;
  constexpr int height = 48;
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
    Gdiplus::RectF handle_rect(0, 0, static_cast<float>(width),
                               static_cast<float>(height));
    Gdiplus::GraphicsPath handle_path;
    AddRoundedRectangle(handle_path, handle_rect, 6.0f);
    Gdiplus::SolidBrush handle_brush(Gdiplus::Color(205, 130, 134, 140));
    graphics.FillPath(&handle_brush, &handle_path);
  }
  RECT bounds{};
  GetWindowRect(drag_handle_, &bounds);
  POINT destination{bounds.left, bounds.top};
  POINT source{0, 0};
  SIZE size{width, height};
  BLENDFUNCTION blend{};
  blend.BlendOp = AC_SRC_OVER;
  blend.SourceConstantAlpha = 255;
  blend.AlphaFormat = AC_SRC_ALPHA;
  UpdateLayeredWindow(drag_handle_, screen_dc, &destination, &size, mem_dc,
                      &source, 0, &blend, ULW_ALPHA);
  SelectObject(mem_dc, previous);
  DeleteObject(dib);
  DeleteDC(mem_dc);
  ReleaseDC(nullptr, screen_dc);
}

void LyricsOverlay::DrawLyricsWheel(Gdiplus::Graphics& graphics, int width,
                                    int height) {
  if (lines_.empty()) return;
  const int index = std::clamp(current_index_, 0,
                               static_cast<int>(lines_.size()) - 1);
  const float left_padding = 44.0f;
  const float right_padding = 24.0f;
  const float text_width = std::max(1.0f, width - left_padding - right_padding);
  const float center_y = height * 0.43f;
  const int font_size = style_.font_size + (style_.zoom_active_line ? 4 : 0);
  Gdiplus::FontFamily font_family(style_.font_family.c_str());
  Gdiplus::Font font(&font_family, static_cast<float>(font_size),
                     Gdiplus::FontStyleBold, Gdiplus::UnitPixel);
  Gdiplus::StringFormat format;
  format.SetFormatFlags(Gdiplus::StringFormatFlagsNoWrap);
  format.SetTrimming(Gdiplus::StringTrimmingEllipsisCharacter);
  format.SetAlignment(
      style_.align == DT_LEFT
          ? Gdiplus::StringAlignmentNear
          : style_.align == DT_RIGHT ? Gdiplus::StringAlignmentFar
                                     : Gdiplus::StringAlignmentCenter);
  format.SetLineAlignment(Gdiplus::StringAlignmentCenter);
  Gdiplus::RectF current_rect(left_padding, center_y - font_size * 0.75f,
                              text_width, font_size * 1.5f);
  const Gdiplus::Color base_color(
      255, GetRValue(style_.text_color), GetGValue(style_.text_color),
      GetBValue(style_.text_color));
  const Gdiplus::Color accent_color(255, 0, 198, 91);
  Gdiplus::SolidBrush base_brush(base_color);
  graphics.DrawString(lines_[index].c_str(), -1, &font, current_rect, &format,
                      &base_brush);

  if (line_progress_ > 0.0f) {
    const Gdiplus::GraphicsState state = graphics.Save();
    Gdiplus::RectF measured;
    graphics.MeasureString(lines_[index].c_str(), -1, &font, current_rect,
                           &format, &measured);
    Gdiplus::RectF clip_rect(measured.X, current_rect.Y,
                            measured.Width * line_progress_, current_rect.Height);
    graphics.SetClip(clip_rect);
    Gdiplus::SolidBrush accent_brush(accent_color);
    graphics.DrawString(lines_[index].c_str(), -1, &font, current_rect, &format,
                        &accent_brush);
    graphics.Restore(state);
  }

  Gdiplus::Font translation_font(&font_family, 16.0f,
                                 Gdiplus::FontStyleRegular,
                                 Gdiplus::UnitPixel);
  Gdiplus::RectF secondary_rect(left_padding, center_y + 20.0f, text_width,
                                28.0f);
  if (style_.show_translation && !current_translation_.empty()) {
    Gdiplus::SolidBrush translation_brush(Gdiplus::Color(215, 225, 228, 232));
    graphics.DrawString(current_translation_.c_str(), -1, &translation_font,
                        secondary_rect, &format, &translation_brush);
  } else if (index + 1 < static_cast<int>(lines_.size())) {
    Gdiplus::SolidBrush next_brush(Gdiplus::Color(155, 225, 228, 232));
    graphics.DrawString(lines_[index + 1].c_str(), -1, &translation_font,
                        secondary_rect, &format, &next_brush);
  }
}

void LyricsOverlay::DrawToolbar(Gdiplus::Graphics& graphics, int width) {
  const int toolbar_width = kToolbarButtonCount * kToolbarButtonWidth +
                            (kToolbarButtonCount - 1) * kToolbarButtonGap;
  const int left = width - toolbar_width - kToolbarRight;
  Gdiplus::RectF toolbar_rect(static_cast<float>(left - 6),
                              static_cast<float>(kToolbarTop - 4),
                              static_cast<float>(toolbar_width + 12), 40.0f);
  Gdiplus::GraphicsPath toolbar_path;
  AddRoundedRectangle(toolbar_path, toolbar_rect, 8.0f);
  Gdiplus::SolidBrush toolbar_brush(Gdiplus::Color(236, 25, 27, 31));
  graphics.FillPath(&toolbar_brush, &toolbar_path);

  const wchar_t* labels[kToolbarButtonCount] = {
      L"字号-", L"字号+", L"背景-", L"背景+", L"放大", L"译文", L"置顶",
      L"锁定", L"关闭"};
  const bool active[kToolbarButtonCount] = {
      false, false, false, false, style_.zoom_active_line,
      style_.show_translation, style_.always_on_top, style_.locked, false};
  Gdiplus::FontFamily family(L"Microsoft YaHei");
  Gdiplus::Font font(&family, 12.0f, Gdiplus::FontStyleBold,
                     Gdiplus::UnitPixel);
  Gdiplus::StringFormat format;
  format.SetAlignment(Gdiplus::StringAlignmentCenter);
  format.SetLineAlignment(Gdiplus::StringAlignmentCenter);
  for (int index = 0; index < kToolbarButtonCount; ++index) {
    const float x = static_cast<float>(left +
        index * (kToolbarButtonWidth + kToolbarButtonGap));
    Gdiplus::RectF button_rect(x, static_cast<float>(kToolbarTop),
                               static_cast<float>(kToolbarButtonWidth), 30.0f);
    Gdiplus::GraphicsPath button_path;
    AddRoundedRectangle(button_path, button_rect, 6.0f);
    Gdiplus::SolidBrush button_brush(active[index]
        ? Gdiplus::Color(255, 48, 107, 83)
        : Gdiplus::Color(210, 53, 57, 62));
    graphics.FillPath(&button_brush, &button_path);
    Gdiplus::SolidBrush label_brush(Gdiplus::Color(255, 245, 246, 247));
    graphics.DrawString(labels[index], -1, &font, button_rect, &format,
                        &label_brush);
  }
}

int LyricsOverlay::ToolbarButtonAt(POINT point, int width) const {
  if (!toolbar_visible_ || style_.locked || point.y < kToolbarTop - 4 ||
      point.y > kToolbarTop + 34) {
    return -1;
  }
  const int toolbar_width = kToolbarButtonCount * kToolbarButtonWidth +
                            (kToolbarButtonCount - 1) * kToolbarButtonGap;
  const int left = width - toolbar_width - kToolbarRight;
  const int offset = point.x - left;
  if (offset < 0) return -1;
  const int stride = kToolbarButtonWidth + kToolbarButtonGap;
  const int index = offset / stride;
  if (index >= kToolbarButtonCount || offset % stride >= kToolbarButtonWidth) {
    return -1;
  }
  return index;
}
