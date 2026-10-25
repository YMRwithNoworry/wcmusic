#ifndef RUNNER_LYRICS_OVERLAY_H_
#define RUNNER_LYRICS_OVERLAY_H_

#include <windows.h>
#include <gdiplus.h>

#include <functional>
#include <string>
#include <vector>

// 系统已安装字体的字族名（UTF-16），可在歌词窗口创建前调用
std::vector<std::wstring> InstalledFontFamilies();

struct LyricsOverlayStyle {
  std::wstring font_family = L"Microsoft YaHei UI";
  // 字体变体：regular / bold / light，对应 GDI+ 的常规、粗体、细体字面
  std::wstring font_weight = L"bold";
  int font_size = 24;
  int align = DT_RIGHT;
  COLORREF text_color = RGB(243, 243, 243);
  // 背景色的 RGB 与 ARGB 透明度分开保存，默认 alpha 为 0，即无底色。
  COLORREF background_color = RGB(0, 0, 0);
  BYTE background_alpha = 0;
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
              float line_progress, const std::wstring& current_translation,
              const std::wstring& next_translation);
  void ApplyStyle(const LyricsOverlayStyle& style);
  void SetPositionCallback(PositionCallback callback);
  void Destroy();

 private:
  static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wparam,
                                     LPARAM lparam);

  void Paint();
  void DrawLyricsWheel(Gdiplus::Graphics& graphics, int width, int height);
  void ApplyWindowAttributes();
  /// 把整个窗口刷成全透明（显示前调用，避免闪出不透明底色）
  void Clear();

  HWND window_ = nullptr;
  std::vector<std::wstring> lines_;
  int current_index_ = 0;
  int displayed_index_ = 0;
  float animation_t_ = 1.0f;
  float line_progress_ = 0.0f;
  // 当前行与下一行的中文翻译
  std::wstring current_translation_;
  std::wstring next_translation_;
  LyricsOverlayStyle style_;
  PositionCallback position_callback_;
};

#endif  // RUNNER_LYRICS_OVERLAY_H_
