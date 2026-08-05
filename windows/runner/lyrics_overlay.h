#ifndef RUNNER_LYRICS_OVERLAY_H_
#define RUNNER_LYRICS_OVERLAY_H_

#include <windows.h>
#include <gdiplus.h>

#include <functional>
#include <string>
#include <vector>

struct LyricsOverlayStyle {
  std::wstring font_family = L"Microsoft YaHei UI";
  int font_size = 30;
  int align = DT_CENTER;
  COLORREF text_color = RGB(245, 243, 236);
  COLORREF background_color = RGB(28, 31, 27);
  int opacity = 224;
  int corner_radius = 18;
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
  void Update(const std::vector<std::wstring>& lines, int current_index);
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
  LyricsOverlayStyle style_;
  PositionCallback position_callback_;
};

#endif  // RUNNER_LYRICS_OVERLAY_H_
