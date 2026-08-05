#ifndef RUNNER_LYRICS_OVERLAY_H_
#define RUNNER_LYRICS_OVERLAY_H_

#include <windows.h>
#include <gdiplus.h>

#include <functional>
#include <string>
#include <vector>

struct LyricsOverlayStyle {
  std::wstring font_family = L"Microsoft YaHei UI";
  int font_size = 24;
  int align = DT_RIGHT;
  COLORREF text_color = RGB(243, 243, 243);
  COLORREF background_color = RGB(20, 20, 22);
  int opacity = 235;
  int corner_radius = 0;
  bool locked = true;
  bool has_position = false;
  int x = 0;
  int y = 0;
};

class LyricsOverlay {
 public:
  using PositionCallback = std::function<void(int x, int y)>;

  LyricsOverlay();
  ~LyricsOverlay();

  bool Create();
  void Show();
  void Hide();
  void Update(const std::vector<std::wstring>& lines, int current_index,
              float line_progress);
  void ApplyStyle(const LyricsOverlayStyle& style);
  void SetPositionCallback(PositionCallback callback);
  void Destroy();

 private:
  static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wparam,
                                     LPARAM lparam);

  void Paint();
  void DrawLyricsWheel(Gdiplus::Graphics& graphics, int width, int height);
  void ApplyWindowAttributes();

  HWND window_ = nullptr;
  std::vector<std::wstring> lines_;
  int current_index_ = 0;
  int displayed_index_ = 0;
  float animation_t_ = 1.0f;
  float line_progress_ = 0.0f;
  LyricsOverlayStyle style_;
  PositionCallback position_callback_;
};

#endif  // RUNNER_LYRICS_OVERLAY_H_
