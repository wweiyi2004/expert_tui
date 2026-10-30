import 'package:expert_tui/data/model_token_limits.dart';
import 'package:expert_tui/data/provider_profile.dart';
import 'package:expert_tui/data/settings_state.dart';
import 'package:expert_tui/domain/llm/llm_provider.dart';
import 'package:expert_tui/state/model_context.dart';
import 'package:test/test.dart';

void main() {
  test('each listed model keeps its own reported efforts', () {
    // Levels as model discovery stores them from a provider's /models reply.
    final profile = ProviderProfile(
      name: 'Gateway',
      baseUrl: 'https://gateway.example.com/v1',
      chatModel: 'alpha',
      reasonerModel: 'alpha',
      models: const ['alpha', 'beta'],
      modelLimits: const {
        'alpha': ModelTokenLimits(reasoningEfforts: ['low', 'high']),
        'beta': ModelTokenLimits(reasoningEfforts: ['minimal', 'medium']),
      },
    );
    final settings = SettingsState(
      profiles: [profile],
      activeProfileId: profile.id,
      apiKey: 'sk-test',
    );
    List<String> efforts(LlmConfig config) =>
        reasoningEffortsFor(config).map((e) => e.wire).toList();

    // The active config pins alpha's reported capabilities; a bare
    // copyWith(model:) carries them over to every other model.
    expect(efforts(settings.config.copyWith(model: 'beta')), ['low', 'high']);

    final derived = applyReportedModelCapabilities(
      settings,
      settings.config.copyWith(model: 'beta', profileId: profile.id),
    );
    expect(efforts(derived), ['minimal', 'medium']);
  });
}
