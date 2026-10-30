import 'dart:convert';

import 'package:mime/mime.dart';

import '../../data/models.dart';

/// Separates binary file bodies from tool text before recording or forwarding it.
class WorkspaceFilePreview {
  WorkspaceFilePreview._(this.metadata, this.image);

  static const maxBytes = 8 * 1024 * 1024;
  static const imageExtensions = {'png', 'jpg', 'jpeg', 'gif', 'webp'};
  static const imageMimeTypes = {
    'image/png',
    'image/jpeg',
    'image/gif',
    'image/webp',
  };

  final Map<String, dynamic> metadata;
  final Attachment? image;

  /// Explicit UTF-8 reads remain available even for image-named source files.
  static void prepareRead(Map<String, dynamic> args) {
    final path = args['path'];
    if (args['encoding'] == null &&
        path is String &&
        imageExtensions.contains(path.split('.').last.toLowerCase())) {
      args['encoding'] = 'base64';
    }
  }

  static WorkspaceFilePreview? parse(Map read) {
    if (read['encoding'] != 'base64') return null;
    final content = read['content'];
    if (content is! String || content.length > ((maxBytes + 2) ~/ 3) * 4) {
      throw const FormatException('文件预览内容无效或超过 8 MiB');
    }
    final bytes = base64Decode(content);
    if (bytes.length > maxBytes || read['bytes'] != bytes.length) {
      throw const FormatException('文件预览大小不匹配或超过 8 MiB');
    }
    final mime = read['mime_type'];
    final path = read['path'];
    if (imageMimeTypes.contains(mime) &&
        (bytes.isEmpty || lookupMimeType('', headerBytes: bytes) != mime)) {
      throw const FormatException('图片内容与声明格式不匹配');
    }
    final image = imageMimeTypes.contains(mime)
        ? Attachment(
            name: path is String
                ? path.split(RegExp(r'[/\\]')).last
                : 'workspace-image',
            mimeType: mime as String,
            sizeBytes: bytes.length,
            imageBase64: base64Encode(bytes),
          )
        : null;
    return WorkspaceFilePreview._({
      for (final key in ['scope', 'path', 'sha256', 'bytes', 'mime_type'])
        if (read.containsKey(key)) key: read[key],
      'encoding': 'base64',
      'content_omitted': true,
    }, image);
  }

  String summary({required bool imageObservation, String? visionAnalysis}) =>
      jsonEncode({
        ...metadata,
        'image_observation': imageObservation,
        'vision_analysis': ?visionAnalysis,
        'note': image == null
            ? '二进制文件已读取并返回元数据；此格式不能作为视觉输入。'
            : imageObservation
            ? '图片像素随本次工具结果单独提供，请根据图片作答。'
            : visionAnalysis != null
            ? '当前聊天模型未收到像素；独立视觉 API 的结果见 vision_analysis。'
            : '当前聊天模型不支持视觉，图片像素未发送；请切换视觉模型或配置视觉 API。',
      });
}
