import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:hydroxy_rc/session/session_storage.dart';
import 'package:hydroxy_rc/configs/web_provider.dart';
import 'package:hydroxy_rc/network/models.dart';

enum DaemonConnectionState {
  connecting,
  connected,
  disconnected,
}

class ApiClient {
  static final ApiClient _instance = ApiClient._internal();
  static ApiClient get instance => _instance;

  String? _authToken;
  String _baseUrl = 'http://127.0.0.1:10000';
  String? _username;
  late final Dio dio;

  bool _hasEverConnected = false;
  int _consecutiveFailures = 0;
  final ValueNotifier<DaemonConnectionState> connectionStateNotifier =
      ValueNotifier<DaemonConnectionState>(DaemonConnectionState.connecting);

  DaemonConnectionState get connectionState => connectionStateNotifier.value;
  bool get isConnected => connectionState == DaemonConnectionState.connected;

  void markConnected() {
    _hasEverConnected = true;
    _consecutiveFailures = 0;
    if (connectionStateNotifier.value != DaemonConnectionState.connected) {
      connectionStateNotifier.value = DaemonConnectionState.connected;
    }
  }

  void markDisconnected() {
    _consecutiveFailures++;
    if (_hasEverConnected && _consecutiveFailures < 2) {
      return;
    }
    final next = _hasEverConnected
        ? DaemonConnectionState.disconnected
        : DaemonConnectionState.connecting;
    if (connectionStateNotifier.value != next) {
      connectionStateNotifier.value = next;
    }
  }

  void resetConnectionState() {
    _hasEverConnected = false;
    _consecutiveFailures = 0;
    connectionStateNotifier.value = DaemonConnectionState.connecting;
  }

  ApiClient._internal() {
    _baseUrl = _detectInitialBaseUrl();
    dio = Dio(BaseOptions(
      baseUrl: _baseUrl,
      connectTimeout: const Duration(seconds: 15),
      receiveTimeout: const Duration(seconds: 15),
    ));

    dio.interceptors.add(InterceptorsWrapper(
      onRequest: (options, handler) {
        options.extra['withCredentials'] = true;
        if (_authToken != null && _authToken!.isNotEmpty) {
          options.headers['Authorization'] = 'Bearer $_authToken';
          options.headers['X-Hydroxy-Token'] = _authToken;
        }
        return handler.next(options);
      },
      onResponse: (response, handler) {
        if (response.statusCode != null && response.statusCode! < 400) {
          markConnected();
        }
        return handler.next(response);
      },
      onError: (DioException error, handler) {
        if (error.type == DioExceptionType.connectionTimeout ||
            error.type == DioExceptionType.sendTimeout ||
            error.type == DioExceptionType.receiveTimeout ||
            error.type == DioExceptionType.connectionError ||
            error.type == DioExceptionType.unknown) {
          markDisconnected();
        }
        return handler.next(error);
      },
    ));
  }

  String get baseUrl => _baseUrl;
  String? get authToken => _authToken;
  String? get username => _username;
  bool get isAuthenticated => _authToken != null && _authToken!.isNotEmpty;

  static String _detectInitialBaseUrl() {
    if (kIsWeb) {
      try {
        final origin = WebUrlDetails.current().origin;
        if (origin.isNotEmpty && origin != 'null') {
          return origin;
        }
      } catch (_) {}
    }
    return 'http://127.0.0.1:10000';
  }

  void setBaseUrl(String url) {
    var trimmed = url.trim();
    if (!trimmed.startsWith('http://') && !trimmed.startsWith('https://')) {
      trimmed = 'http://$trimmed';
    }
    while (trimmed.endsWith('/')) {
      trimmed = trimmed.substring(0, trimmed.length - 1);
    }
    _baseUrl = trimmed;
    dio.options.baseUrl = _baseUrl;
  }

  void restoreSession({
    required String serverHostUrl,
    required String authToken,
    required String username,
  }) {
    setBaseUrl(serverHostUrl);
    _authToken = authToken;
    _username = username;
  }

  Future<bool> login({
    String? serverHostUrl,
    required String username,
    required String password,
  }) async {
    if (serverHostUrl != null && serverHostUrl.trim().isNotEmpty && serverHostUrl != 'serverHostUrl') {
      setBaseUrl(serverHostUrl);
    }

    final loginUrl = '$_baseUrl/api/auth/login';
    try {
      final response = await dio.post(
        loginUrl,
        data: {
          'username': username,
          'password': password,
        },
        options: Options(
          headers: {'Content-Type': 'application/json'},
          validateStatus: (status) => status != null && status < 500,
        ),
      );

      if (response.statusCode == 200 && response.data is Map<String, dynamic>) {
        final data = response.data as Map<String, dynamic>;
        _authToken = data['token']?.toString();
        _username = data['username']?.toString() ?? username;
        return true;
      }

      return false;
    } on DioException catch (e) {
      if (e.response?.statusCode == 401) {
        return false;
      }
      rethrow;
    }
  }

  Future<void> logout() async {
    try {
      if (isAuthenticated) {
        await dio.post(
          '$_baseUrl/api/auth/logout',
          options: Options(validateStatus: (_) => true),
        );
      }
    } catch (_) {
      // Ignore network errors on logout
    } finally {
      _authToken = null;
      _username = null;
      resetConnectionState();
      await SessionStorage.clearSession();
    }
  }

  Future<bool> checkAuthStatus() async {
    try {
      final response = await dio.get(
        '$_baseUrl/api/auth/status',
        options: Options(
          validateStatus: (status) => status != null && status < 500,
          extra: {'withCredentials': true},
        ),
      );
      if (response.statusCode == 200 && response.data is Map<String, dynamic>) {
        final data = response.data as Map<String, dynamic>;
        final token = data['token']?.toString();
        if (token != null && token.isNotEmpty) {
          _authToken = token;
        }
        _username = data['username']?.toString() ?? _username;
        return true;
      }
      return false;
    } catch (_) {
      return false;
    }
  }
}

Future<List<TorrentStructure>> getTorrents({
  String? serverHostUrl,
}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  final response = await client.dio.get('${client.baseUrl}/api/torrents');
  final List<dynamic> data = response.data as List<dynamic>;

  return data
      .map((item) => TorrentStructure.fromJson(item as Map<String, dynamic>))
      .toList();
}

Future<TorrentDetailApiItem> getTorrentDetail({
  required String infoHash,
  String? serverHostUrl,
}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  final response = await client.dio.get('${client.baseUrl}/api/torrent/$infoHash');
  return TorrentDetailApiItem.fromJson(response.data as Map<String, dynamic>);
}

Future<String> getDaemonLogs({String? serverHostUrl}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  final response = await client.dio.get('${client.baseUrl}/api/logs');
  if (response.data is Map<String, dynamic>) {
    return (response.data as Map<String, dynamic>)['logs']?.toString() ?? '';
  }
  return response.data?.toString() ?? '';
}

Future<bool> shutdownDaemon({String? serverHostUrl}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  try {
    final response = await client.dio.post(
      '${client.baseUrl}/api/shutdown',
      options: Options(receiveTimeout: const Duration(seconds: 3)),
    );
    return response.statusCode == 200;
  } catch (_) {
    // If connection was abruptly closed because daemon terminated immediately, treat as success
    return true;
  }
}

Future<SystemStatsModel> getSystemStats({String? serverHostUrl}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  final response = await client.dio.get(
    '${client.baseUrl}/api/system/stats',
    options: Options(receiveTimeout: const Duration(seconds: 10)),
  );
  return SystemStatsModel.fromJson(response.data as Map<String, dynamic>);
}

Future<bool> pauseTorrent({
  required String infoHash,
  String? serverHostUrl,
}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  try {
    final response = await client.dio.post('${client.baseUrl}/api/torrent/$infoHash/pause');
    return response.statusCode == 200;
  } catch (_) {
    return false;
  }
}

Future<bool> resumeTorrent({
  required String infoHash,
  String? serverHostUrl,
}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  try {
    final response = await client.dio.post('${client.baseUrl}/api/torrent/$infoHash/resume');
    return response.statusCode == 200;
  } catch (_) {
    return false;
  }
}

Future<bool> recheckTorrent({
  required String infoHash,
  String? serverHostUrl,
}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  try {
    final response = await client.dio.post('${client.baseUrl}/api/torrent/$infoHash/recheck');
    return response.statusCode == 200;
  } catch (_) {
    return false;
  }
}

Future<bool> reannounceTorrent({
  required String infoHash,
  String? serverHostUrl,
}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  try {
    final response = await client.dio.post('${client.baseUrl}/api/torrent/$infoHash/reannounce');
    return response.statusCode == 200;
  } catch (_) {
    return false;
  }
}

Future<bool> setSequentialDownload({
  required String infoHash,
  required bool sequential,
  String? serverHostUrl,
}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  try {
    final response = await client.dio.post(
      '${client.baseUrl}/api/torrent/$infoHash/sequential',
      data: {'sequential': sequential},
      options: Options(headers: {'Content-Type': 'application/json'}),
    );
    return response.statusCode == 200;
  } catch (_) {
    return false;
  }
}

Future<bool> removeTorrent({
  required String infoHash,
  bool deleteFiles = false,
  String? serverHostUrl,
}) async {
  final client = ApiClient.instance;
  if (serverHostUrl != null &&
      serverHostUrl.trim().isNotEmpty &&
      serverHostUrl != 'serverHostUrl') {
    client.setBaseUrl(serverHostUrl);
  }

  try {
    final response = await client.dio.post(
      '${client.baseUrl}/api/torrent/$infoHash/remove',
      data: {'delete_files': deleteFiles},
      options: Options(
        headers: {'Content-Type': 'application/json'},
        validateStatus: (status) => status != null && status < 500,
      ),
    );
    return response.statusCode == 200;
  } catch (_) {
    return false;
  }
}

String formatConnectionError(dynamic error, [String? fallbackHostUrl]) {
  String target = (fallbackHostUrl != null &&
          fallbackHostUrl.trim().isNotEmpty &&
          fallbackHostUrl != 'serverHostUrl')
      ? fallbackHostUrl.trim()
      : ApiClient.instance.baseUrl;

  if (error is DioException) {
    final uri = error.requestOptions.uri;
    final host = uri.host.isNotEmpty ? uri.host : '127.0.0.1';
    final port = uri.port != 0 ? uri.port : 10000;
    final endpoint = '$host:$port';

    if (error.type == DioExceptionType.connectionTimeout) {
      return 'Connection timed out to $endpoint. The HydroxyTorrent daemon may not be running or reachable.';
    }
    if (error.type == DioExceptionType.receiveTimeout ||
        error.type == DioExceptionType.sendTimeout) {
      return 'Request timed out waiting for daemon response at $endpoint.';
    }
    if (error.type == DioExceptionType.connectionError) {
      return 'Could not connect to HydroxyTorrent daemon at $endpoint.\n'
          'Please verify that hydroxy_daemon is running and listening on that host and port.';
    }
    if (error.response?.statusCode != null) {
      final code = error.response!.statusCode;
      if (code == 401) return 'Invalid username or password.';
      if (code == 404) return 'Daemon API endpoint not found at $endpoint.';
      if (code == 500) return 'Internal server error from daemon at $endpoint.';
      return 'Server error ($code) from daemon at $endpoint.';
    }
  }

  final msg = error.toString();
  if (msg.contains('Connection refused') || msg.contains('SocketException')) {
    try {
      var normalized = target;
      if (!normalized.startsWith('http://') && !normalized.startsWith('https://')) {
        normalized = 'http://$normalized';
      }
      final parsed = Uri.parse(normalized);
      final host = parsed.host.isNotEmpty ? parsed.host : '127.0.0.1';
      final port = parsed.port != 0 ? parsed.port : 10000;
      return 'Could not connect to HydroxyTorrent daemon at $host:$port.\n'
          'Please verify that hydroxy_daemon is running and listening on that host and port.';
    } catch (_) {
      return 'Could not connect to HydroxyTorrent daemon at $target.\n'
          'Please verify that hydroxy_daemon is running and listening on that host and port.';
    }
  }

  return 'Connection failed: $msg';
}
