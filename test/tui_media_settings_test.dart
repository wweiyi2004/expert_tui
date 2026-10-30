import 'dart:convert';
import 'package:expert_tui/core/providers.dart';
import 'package:expert_tui/data/media_api_config.dart';
import 'package:expert_tui/features/tui_bridge/settings_bridge.dart';
import 'package:expert_tui/features/tui_bridge/protocol.dart';
import 'package:expert_tui/state/settings_controller.dart';
import 'package:expert_tui/platform/preferences.dart';
import 'package:expert_tui/platform/credentials.dart';
import 'package:riverpod/riverpod.dart';
import 'package:test/test.dart';
import 'support/runtime_fakes.dart' show FakeSettings;

void main() {

  late ProviderContainer container;
  late TuiSettingsBridge bridge;
  setUp(() async {
    SharedPreferences.setMockInitialValues({});
    FlutterSecureStorage.setMockInitialValues({});
    container = ProviderContainer(
      overrides: [
        sharedPrefsProvider.overrideWithValue(
          await SharedPreferences.getInstance(),
        ),
        settingsControllerProvider.overrideWith(FakeSettings.new),
      ],
    );
    await container.read(settingsControllerProvider.future);
    bridge = TuiSettingsBridge(container, rememberSecrets: (_) {});
  });
  tearDown(() => container.dispose());
  test(
    'all media configurations persist non-secret fields, preserve and explicitly clear keys',
    () async {
      for (final kind in MediaApiKind.values) {
        final response = await bridge.invoke('media.save', {
          'kind': kind.name,
          'base_url': 'https://synthetic.invalid/v1',
          'model': 'image-model',
          'api_key': 'SYNTHETIC_MEDIA_KEY',
          'image_response_format': 'b64_json',
          'enabled': true,
        });
        expect(jsonEncode(response), isNot(contains('SYNTHETIC_MEDIA_KEY')));
        var value = (response['media'] as Map)[kind.name] as Map;
        expect(value['configured'], isTrue);
        final saved = await bridge.invoke('media.save', {
          'kind': kind.name,
          'model': 'new-model',
          'api_key': '',
        });
        expect(((saved['media'] as Map)[kind.name] as Map)['has_key'], isTrue);
        final cleared = await bridge.invoke('media.save', {
          'kind': kind.name,
          'clear_key': true,
        });
        expect(
          ((cleared['media'] as Map)[kind.name] as Map)['configured'],
          isFalse,
        );
      }
      final prefs = container.read(sharedPrefsProvider);
      expect(prefs.getString('imageGenerationApi'), contains('b64_json'));
      expect(
        prefs.getString('imageGenerationApi'),
        isNot(contains('SYNTHETIC_MEDIA_KEY')),
      );
    },
  );
  test('invalid fields fail before changing configuration', () async {
    for (final values in [
      {'base_url': 'https://name:password@example.com/v1'},
      {'api_format': 'anthropicMessages'},
      {'image_response_format': 'unknown'},
      {'clear_key': 'true'},
    ]) {
      await expectLater(
        bridge.invoke('media.save', {
          'kind': 'imageGeneration',
          'base_url': 'https://example.com/v1',
          'model': 'image-1',
          ...values,
        }),
        throwsA(isA<BridgeFailure>()),
      );
      expect(
        container
            .read(settingsControllerProvider)
            .requireValue
            .imageGenerationApi
            .baseUrl,
        isEmpty,
      );
    }
  });
}
