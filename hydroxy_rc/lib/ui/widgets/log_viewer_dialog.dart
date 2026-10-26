import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';

class LogViewerDialog extends StatefulWidget {
  const LogViewerDialog({super.key});

  static Future<void> show(BuildContext context) {
    return showDialog(
      context: context,
      builder: (_) => const LogViewerDialog(),
    );
  }

  @override
  State<LogViewerDialog> createState() => _LogViewerDialogState();
}

class _LogViewerDialogState extends State<LogViewerDialog> {
  final ScrollController _verticalController = ScrollController();
  final ScrollController _horizontalController = ScrollController();
  Timer? _timer;
  String _logs = '';
  bool _isLoading = true;
  bool _autoScroll = true;

  @override
  void initState() {
    super.initState();
    _fetchLogs();
    _timer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (mounted) {
        _fetchLogs(isAuto: true);
      }
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    _verticalController.dispose();
    _horizontalController.dispose();
    super.dispose();
  }

  Future<void> _fetchLogs({bool isAuto = false}) async {
    try {
      final logs = await getDaemonLogs();
      if (mounted) {
        final shouldScroll = _autoScroll && (isAuto || _isLoading);
        setState(() {
          _logs = logs.trim().isEmpty ? 'No logs recorded yet.' : logs;
          _isLoading = false;
        });

        if (shouldScroll) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (_verticalController.hasClients) {
              _verticalController.jumpTo(
                _verticalController.position.maxScrollExtent,
              );
            }
          });
        }
      }
    } catch (e) {
      if (mounted && !_isLoading) {
        // Silently keep existing logs on polling failure
      } else if (mounted) {
        setState(() {
          _logs = 'Failed to fetch logs: $e\nEnsure the daemon is running.';
          _isLoading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.of(context).size;

    return Dialog(
      backgroundColor: const Color(0xFF1B1B1B),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
        side: const BorderSide(color: Color(0xFF333333)),
      ),
      insetPadding: const EdgeInsets.symmetric(horizontal: 40, vertical: 30),
      child: Container(
        width: size.width * 0.85,
        height: size.height * 0.85,
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Header Bar
            Row(
              children: [
                const Icon(Icons.terminal, color: Color(0xFF00FF66), size: 20),
                const SizedBox(width: 8),
                const Text(
                  'Daemon Logs',
                  style: TextStyle(
                    color: Colors.white,
                    fontWeight: FontWeight.bold,
                    fontSize: 14,
                  ),
                ),
                const SizedBox(width: 12),
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 6,
                    vertical: 2,
                  ),
                  decoration: BoxDecoration(
                    color: Colors.white,
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: const Row(
                    children: [
                      Icon(
                        Icons.fiber_manual_record,
                        size: 8,
                        color: Color(0xFF00FF66),
                      ),
                      SizedBox(width: 4),
                      Text(
                        'Live (2s)',
                        style: TextStyle(color: Colors.white70, fontSize: 10),
                      ),
                    ],
                  ),
                ),
                const Spacer(),

                // Copy Button
                IconButton(
                  tooltip: 'Copy all logs',
                  icon: const Icon(Icons.copy, size: 16, color: Colors.white70),
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: _logs));
                    final colors = context.appColors;
                    ScaffoldMessenger.of(context).showSnackBar(
                      SnackBar(
                        content: Text(
                          'Logs copied to clipboard',
                          style: TextStyle(color: colors.snackBarTextColor, fontSize: 13, fontWeight: FontWeight.w500),
                        ),
                        backgroundColor: colors.snackBarBackgroundColor,
                        duration: const Duration(seconds: 2),
                      ),
                    );
                  },
                ),

                // Refresh Button
                IconButton(
                  tooltip: 'Refresh now',
                  icon: const Icon(
                    Icons.refresh,
                    size: 18,
                    color: Colors.white70,
                  ),
                  onPressed: () => _fetchLogs(),
                ),

                // Auto-scroll toggle
                IconButton(
                  tooltip: _autoScroll ? 'Auto-scroll on' : 'Auto-scroll off',
                  icon: Icon(
                    _autoScroll ? Icons.vertical_align_bottom : Icons.pause,
                    size: 18,
                    color: _autoScroll
                        ? const Color(0xFF00FF66)
                        : Colors.white38,
                  ),
                  onPressed: () {
                    setState(() {
                      _autoScroll = !_autoScroll;
                    });
                  },
                ),

                // Close Button
                IconButton(
                  tooltip: 'Close',
                  icon: const Icon(
                    Icons.close,
                    size: 18,
                    color: Colors.white70,
                  ),
                  onPressed: () => Navigator.of(context).pop(),
                ),
              ],
            ),
            const Divider(color: Color(0xFF2E2E2E), height: 16),

            // Log Content Terminal View
            Expanded(
              child: Container(
                decoration: BoxDecoration(
                  color: const Color(0xFF0C0C0C),
                  borderRadius: BorderRadius.circular(4),
                  border: Border.all(color: const Color(0xFF222222)),
                ),
                padding: const EdgeInsets.all(12),
                child: _isLoading
                    ? const Center(
                        child: SizedBox(
                          width: 24,
                          height: 24,
                          child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: Color(0xFF00FF66),
                          ),
                        ),
                      )
                    : Scrollbar(
                        controller: _verticalController,
                        thumbVisibility: true,
                        child: Scrollbar(
                          controller: _horizontalController,
                          thumbVisibility: true,
                          notificationPredicate: (n) => n.depth == 1,
                          child: SingleChildScrollView(
                            controller: _verticalController,
                            child: SingleChildScrollView(
                              controller: _horizontalController,
                              scrollDirection: Axis.horizontal,
                              child: SelectableText(
                                _logs,
                                style: const TextStyle(
                                  color: Color(0xFF00FF66),
                                  fontFamily: 'monospace',
                                  fontSize: 12,
                                  height: 1.4,
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
