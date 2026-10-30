part of 'settings_bridge.dart';

extension TuiMediaSettings on TuiSettingsBridge {
  MediaApiConfig _mediaConfig(MediaApiKind kind) => switch (kind) {
    MediaApiKind.vision => settings.visionApi,
    MediaApiKind.imageGeneration => settings.imageGenerationApi,
    MediaApiKind.asr => settings.asrApi,
    MediaApiKind.tts => settings.ttsApi,
  };
  String _mediaKey(MediaApiKind kind) => switch (kind) {
    MediaApiKind.vision => settings.visionApiKey,
    MediaApiKind.imageGeneration => settings.imageGenerationApiKey,
    MediaApiKind.asr => settings.asrApiKey,
    MediaApiKind.tts => settings.ttsApiKey,
  };

  Map<String, Object?> mediaView() => {
    for (final kind in MediaApiKind.values)
      kind.name: {
        'kind': kind.name,
        'base_url': _mediaConfig(kind).baseUrl,
        'model': _mediaConfig(kind).model,
        'api_format': _mediaConfig(kind).apiFormat.wire,
        'image_size': _mediaConfig(kind).imageSize,
        'image_response_format': _mediaConfig(kind).imageResponseFormat,
        if (kind == MediaApiKind.imageGeneration)
          'mode': settings.imageGenMode.wire,
        'voice': _mediaConfig(kind).voice,
        'speech_protocol': _mediaConfig(kind).speechProtocol.name,
        'voice_design_prompt': _mediaConfig(kind).voiceDesignPrompt,
        'voice_clone_path': _mediaConfig(kind).voiceClonePath,
        'has_key': _mediaKey(kind).isNotEmpty,
        'configured': _mediaConfig(kind).isConfiguredWith(_mediaKey(kind)),
        'enabled': kind != MediaApiKind.asr || settings.asrEnabled,
      },
  };

  Future<Map<String, Object?>> saveMedia(
    String method,
    Map<String, dynamic> p,
  ) async {
    final kind = MediaApiKind.values
        .where((k) => k.name == p['kind'])
        .firstOrNull;
    if (kind == null) throw const BridgeFailure('invalid_params', '未知多媒体服务类型');
    for (final field in [
      'base_url',
      'model',
      'api_key',
      'api_format',
      'image_size',
      'image_response_format',
      'mode',
      'voice',
      'speech_protocol',
      'voice_design_prompt',
      'voice_clone_path',
    ]) {
      if (p.containsKey(field) &&
          (p[field] is! String || (p[field] as String).length > 12000)) {
        throw BridgeFailure('invalid_params', '$field 必须为不超过 12000 字符的文本');
      }
    }
    if (p.containsKey('clear_key') && p['clear_key'] is! bool) {
      throw const BridgeFailure('invalid_params', 'clear_key 必须为布尔值');
    }
    final old = _mediaConfig(kind);
    var supplied = p['api_key'] as String?;
    var base = p['base_url'] as String? ?? old.baseUrl;
    var model = p['model'] as String? ?? old.model;
    var format = p['api_format'] as String? ?? old.apiFormat.wire;
    if (method == 'media.import') {
      final id = requiredString(p, 'profile_id');
      final credentials = await controller.readCurrentProfileCredentials(id);
      if (credentials.profile.authMode != 'apiKey') {
        throw const BridgeFailure(
          'invalid_params',
          '订阅登录不能用作多媒体 API Key；请选择 API Key 提供商',
        );
      }
      supplied = credentials.apiKey;
      base = credentials.profile.baseUrl;
      model = credentials.profile.chatModel;
      format = credentials.profile.apiFormat.wire;
      if (!['auto', 'openAiCompatible', 'geminiNative'].contains(format) &&
          kind != MediaApiKind.vision) {
        format = 'auto';
      }
    }
    base = base.trim();
    model = model.trim();
    final uri = Uri.tryParse(base);
    if (uri == null ||
        !['http', 'https'].contains(uri.scheme) ||
        uri.host.isEmpty ||
        uri.userInfo.isNotEmpty ||
        uri.hasFragment ||
        uri.hasQuery) {
      throw const BridgeFailure(
        'invalid_params',
        'Base URL 须为无内嵌凭据、查询参数和片段的 HTTP(S) 地址',
      );
    }
    if (model.isEmpty || model.length > 500) {
      throw const BridgeFailure('invalid_params', '请填写模型 ID（最多 500 字符）');
    }
    if (!LlmApiFormat.values.any((f) => f.wire == format) ||
        kind == MediaApiKind.imageGeneration &&
            !['auto', 'openAiCompatible', 'geminiNative'].contains(format) ||
        (kind == MediaApiKind.asr || kind == MediaApiKind.tts) &&
            !['auto', 'openAiCompatible'].contains(format)) {
      throw const BridgeFailure('invalid_params', '此多媒体服务不支持所选接口格式');
    }
    final protocol = p['speech_protocol'] as String? ?? old.speechProtocol.name;
    if (!SpeechApiProtocol.values.any((v) => v.name == protocol)) {
      throw const BridgeFailure('invalid_params', '未知语音合成协议');
    }
    final key = p['clear_key'] == true
        ? ''
        : supplied == null || supplied.isEmpty
        ? _mediaKey(kind)
        : supplied;
    rememberSecrets([key]);
    final imageFormat =
        p['image_response_format'] as String? ?? old.imageResponseFormat;
    if (!['auto', 'b64_json', 'url'].contains(imageFormat)) {
      throw const BridgeFailure(
        'invalid_params',
        '图片返回格式须为 auto、b64_json 或 url',
      );
    }
    final mode = p['mode'] as String? ?? settings.imageGenMode.wire;
    if (!ImageGenMode.values.any((v) => v.wire == mode)) {
      throw const BridgeFailure('invalid_params', '生图模式须为 off、auto 或 always');
    }
    final config = old.copyWith(
      baseUrl: base,
      model: model,
      apiFormat: LlmApiFormat.fromWire(format),
      imageSize: p['image_size'] as String?,
      imageResponseFormat: imageFormat,
      voice: p['voice'] as String?,
      speechProtocol: SpeechApiProtocol.fromWire(protocol),
      voiceDesignPrompt: p['voice_design_prompt'] as String?,
      voiceClonePath: p['voice_clone_path'] as String?,
    );
    if (kind == MediaApiKind.tts &&
        old.effectiveSpeechProtocol != config.effectiveSpeechProtocol) {
      await controller.selectTtsProvider(
        config.effectiveSpeechProtocol,
        config,
      );
    }
    await controller.setMediaApiConfig(kind, config);
    if (kind == MediaApiKind.imageGeneration && p.containsKey('mode')) {
      await controller.setImageGenMode(ImageGenModeInfo.fromWire(mode));
    }
    if (supplied?.isNotEmpty == true || p['clear_key'] == true) {
      await controller.setMediaApiKey(kind, key);
    }
    if (kind == MediaApiKind.asr && p.containsKey('enabled')) {
      await controller.setAsrEnabled(p['enabled'] == true);
    }
    return {'accepted': true, ...await view()};
  }
}
