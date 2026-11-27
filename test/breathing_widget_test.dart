import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/ui/core/breathing_widget.dart';

void main() {
  testWidgets(
    'repeating player animations settle when disabled',
    (tester) async {
      await tester.pumpWidget(
        const MaterialApp(
          home: MediaQuery(
            data: MediaQueryData(disableAnimations: true),
            child: Column(
              children: [
                BreathingWidget(child: Text('Breathing')),
                PulseWidget(child: Text('Pulse')),
              ],
            ),
          ),
        ),
      );

      await tester.pumpAndSettle();

      expect(find.text('Breathing'), findsOneWidget);
      expect(find.text('Pulse'), findsOneWidget);
    },
  );
}
