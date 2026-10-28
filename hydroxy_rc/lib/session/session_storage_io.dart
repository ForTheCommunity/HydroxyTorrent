import 'dart:convert';
import 'dart:io';

import 'session_models.dart';

class SessionStorage {
  static const String appName = 'torrentsdigger';
  static const String packagePrefix = 'io.gitlab.ForTheCommunity';
  static const String packageName = '$packagePrefix.$appName';

  static Directory getApplicationSupportDirectory() {
    if (Platform.isLinux) {
      final xdgData = Platform.environment['XDG_DATA_HOME'];
      if (xdgData != null && xdgData.trim().isNotEmpty) {
        return Directory('$xdgData/$packageName');
      }
      final home = Platform.environment['HOME'] ?? '.';
      return Directory('$home/.local/share/$packageName');
    } else if (Platform.isMacOS) {
      final home = Platform.environment['HOME'] ?? '.';
      return Directory('$home/Library/Application Support/$packageName');
    } else if (Platform.isWindows) {
      final appData = Platform.environment['APPDATA'] ??
          Platform.environment['LOCALAPPDATA'] ??
          '.';
      return Directory('$appData\\$packageName');
    } else if (Platform.isAndroid) {
      final candidate = Directory('/data/user/0/$packageName/files');
      if (candidate.existsSync()) {
        return candidate;
      }
      final legacy = Directory('/data/data/$packageName/files');
      if (legacy.existsSync()) {
        return legacy;
      }
      return candidate;
    } else if (Platform.isIOS) {
      final home = Platform.environment['HOME'] ?? '.';
      return Directory('$home/Library/Application Support/$packageName');
    }
    return Directory('./.$packageName');
  }

  static File get _sessionFile {
    final dir = getApplicationSupportDirectory();
    if (!dir.existsSync()) {
      try {
        dir.createSync(recursive: true);
      } catch (_) {}
    }
    return File('${dir.path}/session.json');
  }

  static Future<SavedSession?> loadSession() async {
    try {
      final file = _sessionFile;
      if (!await file.exists()) return null;
      final content = await file.readAsString();
      if (content.trim().isEmpty) return null;
      final map = jsonDecode(content) as Map<String, dynamic>;
      final session = SavedSession.fromJson(map);
      if (session.authToken.isEmpty) return null;
      return session;
    } catch (_) {
      return null;
    }
  }

  static Future<void> saveSession(SavedSession session) async {
    try {
      final file = _sessionFile;
      await file.writeAsString(jsonEncode(session.toJson()), flush: true);
    } catch (_) {}
  }

  static Future<void> clearSession() async {
    try {
      final file = _sessionFile;
      if (await file.exists()) {
        await file.delete();
      }
    } catch (_) {}
  }
}
