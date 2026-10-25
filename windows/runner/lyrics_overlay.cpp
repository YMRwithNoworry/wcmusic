#include "lyrics_overlay.h"

#include <algorithm>
#include <cmath>
#include <cstring>
#include <gdiplus.h>
#include <windowsx.h>

#pragma comment(lib, "gdiplus.lib")

namespace {
constexpr wchar_t kWindowClassName[] = L"WCMusicLyricsOverlay";
constexpr int kOverlayWidth = 480;
constexpr int kDragHandleWidth = 18;
constexpr UINT_PTR kLyricsTimerId = 0x4C59;
constexpr ULONGLONG kLyricsAnimationDurationMs = 380;
const std::wstring kEmptyText;

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

// 列出系统已安装字体的字族名，结果在进程内缓存
std::vector<std::wstring> CollectInstalledFontFamilies() {
  static const std::vector<std::wstring> families = [] {
    std::vector<std::wstring> result;
    ULONG_PTR token = 0;
    Gdiplus::GdiplusStartupInput input;
    if (Gdiplus::GdiplusStartup(&token, &input, nullptr) != Gdiplus::Ok) {
      return result;
    }
    Gdiplus::InstalledFontCollection collection;
    const int count = collection.GetFamilyCount();
    if (count <= 0) return result;
    std::vector<Gdiplus::FontFamily> buffer(static_cast<size_t>(count));
    int found = 0;
    collection.GetFamilies(count, buffer.data(), &found);
    result.reserve(static_cast<size_t>(found));
    for (int index = 0; index < found; ++index) {
      WCHAR name[LF_FACESIZE] = {};
      if (buffer[static_cast<size_t>(index)].GetFamilyName(name) ==
              Gdiplus::Ok &&
          name[0] != L'\0') {
        result.emplace_back(name);
      }
    }
    // 启动令牌不释放：GDI+ 在整个进程生命周期内保持可用
    return result;
  }();
  return families;
}

// 把界面上的字体变体映射为 GDI+ 字面；want_regular 为真时忽略变体，使用常规字面
Gdiplus::FontStyle ResolveFontStyle(Gdiplus::FontFamily& family,
                                    const std::wstring& weight,
                                    bool want_regular) {
  const auto can = [&family](Gdiplus::FontStyle style) {
    return family.IsStyleAvailable(style) == Gdiplus::Ok;
  };
  const bool want_bold = !want_regular && weight == L"bold";
  // 字体未必提供全部字面，缺失时退回另一种字重，避免回落到系统字体
  const Gdiplus::FontStyle primary = want_bold ? Gdiplus::FontStyleBold
                                               : Gdiplus::FontStyleRegular;
  const Gdiplus::FontStyle fallback = want_bold ? Gdiplus::FontStyleRegular
                                                : Gdiplus::FontStyleBold;
  if (can(primary)) return primary;
  if (can(fallback)) return fallback;
  return Gdiplus::FontStyleRegular;
}
}  // namespace

// 头文件里声明的公共接口，必须放在匿名命名空间之外，否则其它编译单元链接不到
std::vector<std::wstring> InstalledFontFamilies() {
  return CollectInstalledFontFamilies();
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
  Clear();
  ShowWindow(window_, SW_SHOWNOACTIVATE);
  SetWindowPos(window_, HWND_TOPMOST, 0, 0, 0, 0,
               SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
}

void LyricsOverlay::Hide() {
  if (window_) ShowWindow(window_, SW_HIDE);
}

void LyricsOverlay::Update(
    const std::optional<std::vector<std::wstring>>& lines, int current_index,
    float line_progress, const std::wstring& current_translation,
    const std::wstring& next_translation) {
  const bool lyrics_changed = lines.has_value() && lines_ != *lines;
  if (lines.has_value()) lines_ = *lines;
  const int next_index = lines_.empty()
      ? 0
      : std::clamp(current_index, 0, static_cast<int>(lines_.size()) - 1);
  const bool index_changed = next_index != current_index_;
  line_progress_ = std::clamp(line_progress, 0.0f, 1.0f);
  current_translation_ = current_translation;
  next_translation_ = next_translation;
  if (lines_.empty()) {
    current_index_ = 0;
    displayed_index_ = 0.0f;
    animation_t_ = 1.0f;
    if (window_) {
      KillTimer(window_, kLyricsTimerId);
      InvalidateRect(window_, nullptr, TRUE);
    }
    return;
  }
  if (lyrics_changed) {
    // 新歌或歌词内容变化时从当前行开始，避免沿用上一首歌的滚动位置。
    current_index_ = next_index;
    displayed_index_ = static_cast<float>(next_index);
    animation_t_ = 1.0f;
    animation_start_time_ = 0;
    if (window_) KillTimer(window_, kLyricsTimerId);
  } else if (index_changed) {
    if (animation_t_ < 1.0f) {
      displayed_index_ = Lerp(displayed_index_,
                              static_cast<float>(current_index_),
                              EaseInOutCubic(animation_t_));
    } else {
      displayed_index_ = static_cast<float>(current_index_);
    }
    current_index_ = next_index;
    animation_start_time_ = GetTickCount64();
    animation_t_ = 0.0f;
  }
  if (window_) {
    if (index_changed && !lyrics_changed) {
      SetTimer(window_, kLyricsTimerId, 16, nullptr);
    }
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
        const ULONGLONG elapsed =
            GetTickCount64() - overlay->animation_start_time_;
        overlay->animation_t_ = std::min(
            1.0f, static_cast<float>(elapsed) /
                     static_cast<float>(kLyricsAnimationDurationMs));
        if (overlay->animation_t_ >= 1.0f) {
          overlay->animation_t_ = 1.0f;
          overlay->displayed_index_ =
              static_cast<float>(overlay->current_index_);
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

    if (style_.background_alpha > 0) {
      Gdiplus::RectF full(0, 0, static_cast<float>(width),
                          static_cast<float>(height));
      Gdiplus::GraphicsPath background_path;
      AddRoundedRectangle(
          background_path, full, static_cast<float>(style_.corner_radius));
      const BYTE background_alpha = static_cast<BYTE>(
          style_.opacity * style_.background_alpha / 255);
      Gdiplus::Color background(
          background_alpha, GetRValue(style_.background_color),
          GetGValue(style_.background_color),
          GetBValue(style_.background_color));
      Gdiplus::SolidBrush background_brush(background);
      graphics.FillPath(&background_brush, &background_path);
    }

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

void LyricsOverlay::Clear() {
  if (!window_) return;
  RECT bounds{};
  GetClientRect(window_, &bounds);
  const int width = bounds.right - bounds.left;
  const int height = bounds.bottom - bounds.top;
  if (width <= 0 || height <= 0) return;
  // 窗口显示前先整块刷成全透明，避免首帧闪一下不透明的默认底色。
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
  HBITMAP dib = CreateDIBSection(screen_dc, &bitmap_info, DIB_RGB_COLORS,
                                 &bits, nullptr, 0);
  HGDIOBJ previous = SelectObject(mem_dc, dib);
  std::memset(bits, 0, static_cast<size_t>(width) * height * 4);
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
      ? Lerp(displayed_index_, static_cast<float>(current_index_),
             EaseInOutCubic(animation_t_))
      : static_cast<float>(current_index_);
  const float center_y = height * 0.54f;
  const float spacing = std::max(60.0f, style_.font_size * 2.25f);
  const float max_distance = 3.0f;
  const int first_index = std::max(
      0, static_cast<int>(std::floor(base - max_distance)));
  const int last_index = std::min(
      static_cast<int>(lines_.size()) - 1,
      static_cast<int>(std::ceil(base + max_distance)));
  const float left_padding = 34.0f;
  const float right_padding = 24.0f;
  const float line_width = width - left_padding - right_padding;

  Gdiplus::FontFamily font_family(style_.font_family.c_str());
  const Gdiplus::Color text_color(
      255, GetRValue(style_.text_color), GetGValue(style_.text_color),
      GetBValue(style_.text_color));
  const Gdiplus::Color highlight_color(255, 123, 231, 165);
  const Gdiplus::Color karaoke_color(255, 0, 198, 91);

  for (int index = first_index; index <= last_index; ++index) {
    const float offset = static_cast<float>(index) - base;
    const float distance = std::abs(offset);
    if (distance > max_distance) continue;
    const float y = center_y + offset * spacing;
    if (y < -spacing || y > height + spacing) continue;

    const float focus = EaseInOutCubic(
        1.0f - std::clamp(distance / 1.35f, 0.0f, 1.0f));
    const float distance_fade = std::clamp(1.0f - distance * 0.10f,
                                           0.32f, 1.0f);
    const float edge_fade = std::clamp(
        std::min(y / (spacing * 1.5f),
                 (height - y) / (spacing * 1.5f)),
        0.18f, 1.0f);
    const float font_size = style_.font_size + 5.0f * focus;
    const BYTE alpha = static_cast<BYTE>(std::clamp(
        (108.0f + 147.0f * focus) * distance_fade * edge_fade, 0.0f,
        255.0f));
    const auto blend_channel = [focus](BYTE from, BYTE to) {
      return static_cast<BYTE>(from + (static_cast<int>(to) - from) * focus);
    };
    const Gdiplus::Color line_color(
        alpha,
        blend_channel(text_color.GetRed(), highlight_color.GetRed()),
        blend_channel(text_color.GetGreen(), highlight_color.GetGreen()),
        blend_channel(text_color.GetBlue(), highlight_color.GetBlue()));

    const Gdiplus::FontStyle font_style = ResolveFontStyle(
        font_family, style_.font_weight, /*want_regular=*/focus < 0.78f);
    Gdiplus::Font font(&font_family, font_size, font_style,
                       Gdiplus::UnitPixel);
    Gdiplus::SolidBrush line_brush(line_color);
    Gdiplus::StringFormat format;
    format.SetFormatFlags(Gdiplus::StringFormatFlagsNoWrap);
    format.SetTrimming(Gdiplus::StringTrimmingEllipsisCharacter);
    format.SetAlignment(
        style_.align == DT_LEFT
            ? Gdiplus::StringAlignmentNear
            : style_.align == DT_RIGHT ? Gdiplus::StringAlignmentFar
                                       : Gdiplus::StringAlignmentCenter);
    format.SetLineAlignment(Gdiplus::StringAlignmentCenter);

    Gdiplus::RectF text_rect(left_padding, y - spacing / 2.0f, line_width,
                             spacing);
    const std::wstring& translation = index == current_index_
        ? current_translation_
        : index == current_index_ + 1 ? next_translation_ : kEmptyText;
    const bool has_translation = !translation.empty();
    if (has_translation) text_rect.Y -= spacing * 0.12f;

    Gdiplus::SolidBrush shadow_brush(
        Gdiplus::Color(static_cast<BYTE>(alpha * 0.34f), 0, 0, 0));
    Gdiplus::RectF shadow_rect = text_rect;
    shadow_rect.Y += 1.2f;
    graphics.DrawString(lines_[index].c_str(), -1, &font, shadow_rect, &format,
                        &shadow_brush);
    graphics.DrawString(lines_[index].c_str(), -1, &font, text_rect, &format,
                        &line_brush);

    if (index == current_index_ && line_progress_ > 0.0f) {
      const Gdiplus::GraphicsState state = graphics.Save();
      Gdiplus::RectF measured;
      graphics.MeasureString(lines_[index].c_str(), -1, &font, text_rect,
                             &format, &measured);
      Gdiplus::RectF clip_rect(measured.X, text_rect.Y,
                               measured.Width * line_progress_, spacing);
      graphics.SetClip(clip_rect);
      Gdiplus::SolidBrush karaoke_brush(Gdiplus::Color(
          alpha, karaoke_color.GetRed(), karaoke_color.GetGreen(),
          karaoke_color.GetBlue()));
      graphics.DrawString(lines_[index].c_str(), -1, &font, text_rect, &format,
                          &karaoke_brush);
      graphics.Restore(state);
    }

    if (has_translation) {
      // 翻译行保持清晰的白色层级，不跟随原文的高亮颜色。
      const Gdiplus::FontStyle translation_weight = ResolveFontStyle(
          font_family, style_.font_weight, /*want_regular=*/true);
      Gdiplus::Font translation_font(&font_family, font_size * 0.58f,
                                     translation_weight, Gdiplus::UnitPixel);
      const BYTE translation_alpha = static_cast<BYTE>(alpha * 0.76f);
      Gdiplus::SolidBrush translation_brush(Gdiplus::Color(
          translation_alpha, text_color.GetRed(), text_color.GetGreen(),
          text_color.GetBlue()));
      Gdiplus::RectF translation_rect(
          left_padding, y + spacing * 0.08f, line_width, spacing * 0.48f);
      Gdiplus::SolidBrush translation_shadow_brush(Gdiplus::Color(
          static_cast<BYTE>(translation_alpha * 0.34f), 0, 0, 0));
      Gdiplus::RectF translation_shadow_rect = translation_rect;
      translation_shadow_rect.Y += 1.0f;
      graphics.DrawString(translation.c_str(), -1, &translation_font,
                          translation_shadow_rect, &format,
                          &translation_shadow_brush);
      graphics.DrawString(translation.c_str(), -1, &translation_font,
                          translation_rect, &format, &translation_brush);
    }
  }
}
