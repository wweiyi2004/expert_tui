import 'dart:async';
import 'package:dio/dio.dart';
import 'package:drift/native.dart';
import 'package:expert_tui/core/providers.dart';
import 'package:expert_tui/core/desktop_providers.dart';
import 'package:expert_tui/data/db/app_database.dart' show AppDatabase;
import 'package:expert_tui/data/provider_profile.dart';
import 'package:expert_tui/domain/desktop/workspace_host_stub.dart' as stub;
import 'package:expert_tui/domain/llm/llm_provider.dart';
import 'package:expert_tui/domain/llm/llm_http_errors.dart';
import 'package:expert_tui/state/chat_controller.dart';
import 'package:expert_tui/state/model_service_credentials.dart';
import 'package:expert_tui/state/settings_controller.dart';
import 'package:riverpod/riverpod.dart';
import 'package:test/test.dart';
import 'package:expert_tui/platform/preferences.dart';
import 'support/runtime_fakes.dart' show FakeSettings, InMemoryRepo;

class _NoCredentialAccess extends FakeSettings {
  static int channelCount = 1;
  int credentialReads = 0;
  @override
  Future<SettingsState> build() async {
    final s = await super.build();
    return s.copyWith(
      profiles: [
        ...s.profiles,
        if (channelCount > 1)
          ProviderProfile(
            id: 'second',
            name: 'Second',
            baseUrl: 'https://second.invalid',
            chatModel: s.model,
            reasonerModel: s.model,
            models: [s.model],
          ),
      ],
    );
  }

  @override
  Future<ModelServiceCredentialSnapshot> readCurrentProfileCredentials(
    String id,
  ) async {
    credentialReads++;
    throw StateError(
      'The injected LlmProvider owns credentials; admission must not read them',
    );
  }
}

class _TransientLlm implements LlmProvider {
  int attempts = 0;
  @override
  Stream<ChatChunk> streamChat({
    required LlmConfig config,
    required List<LlmRequestMessage> messages,
    List<ToolSpec>? tools,
    bool? thinking,
    ReasoningEffort? reasoningEffort,
    String? forceToolName,
    CancelToken? cancelToken,
  }) async* {
    attempts++;
    if (attempts == 1) {
      throw const LlmHttpException('temporary', statusCode: 503);
    }
    yield const ChatChunk(contentDelta: 'done', finishReason: 'stop');
  }
}

void main() {

  for (final channels in [1, 2]) {
    test(
      'retry admission uses routing metadata without credential I/O ($channels channels)',
      () async {
        _NoCredentialAccess.channelCount = channels;
        SharedPreferences.setMockInitialValues({});
        final db = AppDatabase(NativeDatabase.memory()), llm = _TransientLlm();
        final container = ProviderContainer(
          overrides: [
            appDatabaseProvider.overrideWithValue(db),
            sharedPrefsProvider.overrideWithValue(
              await SharedPreferences.getInstance(),
            ),
            conversationRepositoryProvider.overrideWithValue(InMemoryRepo()),
            desktopWorkspaceHostProvider.overrideWithValue(
              stub.createDesktopWorkspaceHost(),
            ),
            settingsControllerProvider.overrideWith(_NoCredentialAccess.new),
            llmProvider.overrideWithValue(llm),
          ],
        );
        try {
          await container.read(chatControllerProvider.future);
          final controller = container.read(chatControllerProvider.notifier);
          final id = await controller.createWorkSession();
          await controller.sendToSession(id, 'test injected provider');
          expect(llm.attempts, channels == 1 ? 2 : 1);
          expect(
            (container.read(settingsControllerProvider.notifier)
                    as _NoCredentialAccess)
                .credentialReads,
            0,
          );
          final messages = container
              .read(chatControllerProvider)
              .requireValue
              .conversations
              .firstWhere((c) => c.id == id)
              .activePath;
          if (channels == 1) {
            expect(messages.last.content, 'done');
          } else {
            expect(controller.errorForSession(id), isNotNull);
          }
        } finally {
          container.dispose();
          await Future<void>.delayed(Duration.zero);
          await db.close();
        }
      },
    );
  }
}
