import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/ui/desktop/player/lyrics_view.dart';

void main() {
  testWidgets('karaoke text fills from left to right with progress', (
    tester,
  ) async {
    final progress = ValueNotifier<double>(0);
    addTearDown(progress.dispose);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Align(
          alignment: Alignment.topLeft,
          child: SizedBox(
            width: 300,
            child: KaraokeText(
              text: '月色如水',
              progress: progress,
              style: const TextStyle(fontSize: 20),
              filled: const Color(0xFFE8B667),
              unfilled: const Color(0xFF888888),
            ),
          ),
        ),
      ),
    );

    Offset clipped() {
      final clip = tester.widget<ClipRect>(find.byType(ClipRect));
      return clip.clipper!.getClip(const Size(200, 30)).bottomRight;
    }

    expect(clipped().dx, 0);
    progress.value = .5;
    await tester.pump();
    expect(clipped().dx, closeTo(100, .01));
    progress.value = 1;
    await tester.pump();
    expect(clipped().dx, closeTo(200, .01));
  });
}
