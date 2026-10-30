import 'dart:async';
import 'package:dio/dio.dart';
import 'package:expert_tui/data/conversation_repository.dart';
import 'package:expert_tui/data/gateway_config.dart';
import 'package:expert_tui/data/models.dart';
import 'package:expert_tui/data/provider_profile.dart';
import 'package:expert_tui/domain/llm/llm_provider.dart';
import 'package:expert_tui/state/chat_controller.dart';
import 'package:expert_tui/state/settings_controller.dart';

class FakeLlmProvider implements LlmProvider {
  FakeLlmProvider(
    this.chunks, {
    this.scriptedChunks,
    this.separateAuxiliary = false,
  });

  final List<ChatChunk> chunks;
  final List<List<ChatChunk>>? scriptedChunks;
  final bool separateAuxiliary;
  final List<List<LlmRequestMessage>> calls = [];
  final List<List<LlmRequestMessage>> auxiliaryRequests = [];
  final List<List<ToolSpec>?> toolCalls = [];
  final List<LlmConfig> configs = [];
  LlmConfig? lastConfig;
  List<ToolSpec>? lastTools;
  String? lastForceToolName;
  final List<String?> forceToolNames = [];
  bool? lastThinking;
  ReasoningEffort? lastReasoningEffort;
  int callCount = 0;

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
    final title = messages.firstOrNull?.content == automaticTitleInstruction;
    if (title ||
        separateAuxiliary &&
            config.routingSessionId?.contains(':public-checkpoint:') == true) {
      auxiliaryRequests.add(List<LlmRequestMessage>.of(messages));
      if (title) yield const ChatChunk(contentDelta: '测试会话');
      return;
    }
    final callIndex = callCount++;
    calls.add(List<LlmRequestMessage>.of(messages));
    toolCalls.add(tools == null ? null : List<ToolSpec>.of(tools));
    configs.add(config);
    lastConfig = config;
    lastTools = tools;
    lastForceToolName = forceToolName;
    forceToolNames.add(forceToolName);
    lastThinking = thinking;
    lastReasoningEffort = reasoningEffort;
    final scripts = scriptedChunks;
    final scriptIndex = scripts == null
        ? 0
        : (callIndex < scripts.length ? callIndex : scripts.length - 1);
    final activeChunks = scripts == null ? chunks : scripts[scriptIndex];
    for (final c in activeChunks) {
      yield c;
    }
  }
}

class InMemoryRepo
    with ConversationRepositoryViaLoadAll
    implements ConversationRepository {
  List<Conversation> store = [];
  @override
  Future<List<Conversation>> loadAll() async => store;
  @override
  Future<void> saveAll(List<Conversation> conversations) async =>
      store = conversations;
  @override
  Future<void> saveConversation(Conversation c) async {
    final idx = store.indexWhere((x) => x.id == c.id);
    if (idx >= 0) {
      store[idx] = c;
    } else {
      store.insert(0, c);
    }
  }

  @override
  Future<void> deleteConversation(String id) async =>
      store.removeWhere((c) => c.id == id);
}

class FakeSettings extends SettingsController {
  @override
  Future<SettingsState> build() async {
    final profile = ProviderProfile(
      name: 'DeepSeek',
      baseUrl: 'https://api.deepseek.com',
      chatModel: KnownModels.chat,
      reasonerModel: KnownModels.reasoner,
      models: const [KnownModels.chat, KnownModels.reasoner],
    );
    return SettingsState(
      profiles: [profile],
      activeProfileId: profile.id,
      apiKey: 'sk-test',
      searchApiKey: 'search-key',
      gateway: const GatewayConfig(
        enabled: true,
        baseUrl: 'https://gateway.example.com',
        uploadBaseUrl: 'https://upload.example.com',
        capabilitiesDiscovered: true,
        capabilities: [
          GatewayCapabilityIds.longTasks,
          GatewayCapabilityIds.documentEdit,
          GatewayCapabilityIds.documentConvert,
        ],
      ),
      gatewayToken: 'gateway-token',
    );
  }
}

/// Settings with the factory chat-skill catalog so [_generate] can route.
