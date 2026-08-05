#ifndef RUNNER_LYRICS_OVERLAY_H_
#define RUNNER_LYRICS_OVERLAY_H_

#include <windows.h>

#include <string>

class LyricsOverlay {
 public:
  LyricsOverlay();
  ~LyricsOverlay();

  bool Create();
  void Show();
  void Hide();
  void Update(const std::string& current_line, const std::string& next_line);
  void Destroy();

 private:
  static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wparam,
                                     LPARAM lparam);
  static std::wstring Utf8ToWide(const std::string& value);

  void Paint();

  HWND window_ = nullptr;
  std::wstring current_line_ = L"WCMusic";
  std::wstring next_line_;
};

#endif  // RUNNER_LYRICS_OVERLAY_H_
