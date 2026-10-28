class SavedSession {
  final String serverHostUrl;
  final String authToken;
  final String username;

  const SavedSession({
    required this.serverHostUrl,
    required this.authToken,
    required this.username,
  });

  Map<String, dynamic> toJson() => {
    'server_host_url': serverHostUrl,
    'auth_token': authToken,
    'username': username,
  };

  factory SavedSession.fromJson(Map<String, dynamic> json) {
    return SavedSession(
      serverHostUrl: json['server_host_url'] as String? ?? 'http://127.0.0.1:10000',
      authToken: json['auth_token'] as String? ?? '',
      username: json['username'] as String? ?? '',
    );
  }
}
