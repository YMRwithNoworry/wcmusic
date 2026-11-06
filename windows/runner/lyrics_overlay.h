#ifndef RUNNER_LYRICS_OVERLAY_H_
#define RUNNER_LYRICS_OVERLAY_H_

#include <windows.h>
#include <gdiplus.h>

#include <functional>
#include <string>
#include <vector>

struct LyricsOverlayStyle {
  std::wstring font_family = L"Microsoft YaHei";
  int font_size = 24;
  int align = DT_CENTER;
  COLORREF text_color = RGB(243, 243, 243);
  COLORREF background_color = RGB(20, 20, 22);
  int opacity = 235;
  int corner_radius = 8;
  bool locked = false;
  bool always_on_top = true;
  bool show_translation = true;
  bool zoom_active_line = true;
  int window_width = 1000;
  int window_height = 140;
  bool has_position = false;
  int x = 0;
  int y = 0;
};

class LyricsOverlay {
 public:
  using PositionCallback = std::function<void(int x, int y, int width, int height)>;
  using ActionCallback = std::function<void(const std::string& action)>;

  LyricsOverlay();
  ~LyricsOverlay();

  bool Create();
  void Show();
  void Hide();
  void Update(const std::vector<std::wstring>& lines, int current_index,
              float line_progress, const std::wstring& translation);
  void ApplyStyle(const LyricsOverlayStyle& style);
  void SetPositionCallback(PositionCallback callback);
  void SetActionCallback(ActionCallback callback);
  void Destroy();

 private:
  static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wparam,
                                     LPARAM lparam);

  void Paint();
  void DrawLyricsWheel(Gdiplus::Graphics& graphics, int width, int height);
  void DrawToolbar(Gdiplus::Graphics& graphics, int width);
  void PaintHandle();
  void UpdateHandlePosition();
  int ToolbarButtonAt(POINT point, int width) const;
  void ApplyWindowAttributes();

  HWND window_ = nullptr;
  HWND drag_handle_ = nullptr;
  std::vector<std::wstring> lines_;
  int current_index_ = 0;
  float line_progress_ = 0.0f;
  std::wstring current_translation_;
  LyricsOverlayStyle style_;
  PositionCallback position_callback_;
  ActionCallback action_callback_;
  bool toolbar_visible_ = false;
  bool tracking_mouse_leave_ = false;
};

#endif  // RUNNER_LYRICS_OVERLAY_H_
