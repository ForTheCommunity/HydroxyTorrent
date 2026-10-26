/// Stub implementation for non-web platforms.
/// This file is imported when `dart:js_interop` is not available.

class WebUrlDetails {
  final String fullUrl;
  final String hostname;
  final String port;
  final String protocol;
  final String origin;
  final String pathname;

  const WebUrlDetails({
    required this.fullUrl,
    required this.hostname,
    required this.port,
    required this.protocol,
    required this.origin,
    required this.pathname,
  });

  factory WebUrlDetails.current() {
    throw UnsupportedError(
      'WebUrlDetails.current() is not supported on this platform.',
    );
  }

  String buildApiUrl({String endpoint = '/api'}) {
    return '$origin$endpoint';
  }
}
