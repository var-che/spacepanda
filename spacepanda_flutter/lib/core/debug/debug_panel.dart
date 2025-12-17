import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../providers/api_providers.dart';

/// Debug panel for monitoring app state and gRPC communication
class DebugPanel extends ConsumerStatefulWidget {
  const DebugPanel({super.key});

  @override
  ConsumerState<DebugPanel> createState() => _DebugPanelState();
}

class _DebugPanelState extends ConsumerState<DebugPanel> {
  String _logs = '';
  bool _isExpanded = false;

  @override
  void initState() {
    super.initState();
    _loadLogs();
  }

  Future<void> _loadLogs() async {
    try {
      final file = File('/tmp/spacepanda_flutter_debug.log');
      if (await file.exists()) {
        final content = await file.readAsString();
        final lines = content.split('\n');
        setState(() {
          _logs = lines.take(100).join('\n'); // Last 100 lines
        });
      }
    } catch (e) {
      setState(() {
        _logs = 'Error loading logs: $e';
      });
    }
  }

  Future<void> _clearLogs() async {
    try {
      final file = File('/tmp/spacepanda_flutter_debug.log');
      if (await file.exists()) {
        await file.delete();
      }
      setState(() {
        _logs = '';
      });
    } catch (e) {
      setState(() {
        _logs = 'Error clearing logs: $e';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final sessionToken = ref.watch(sessionTokenProvider);
    final isAuthenticated = ref.watch(isAuthenticatedProvider);

    return Container(
      color: Colors.black87,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Header
          InkWell(
            onTap: () => setState(() => _isExpanded = !_isExpanded),
            child: Container(
              padding: const EdgeInsets.all(8),
              color: Colors.grey[900],
              child: Row(
                children: [
                  Icon(
                    _isExpanded ? Icons.expand_more : Icons.expand_less,
                    color: Colors.white,
                  ),
                  const SizedBox(width: 8),
                  const Text(
                    '🐼 Debug Panel',
                    style: TextStyle(
                      color: Colors.white,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  const Spacer(),
                  _StatusIndicator(
                    label: 'Auth',
                    value: isAuthenticated ? 'Yes' : 'No',
                    color: isAuthenticated ? Colors.green : Colors.red,
                  ),
                  const SizedBox(width: 16),
                  IconButton(
                    icon: const Icon(Icons.refresh, color: Colors.white),
                    onPressed: _loadLogs,
                    tooltip: 'Refresh logs',
                  ),
                  IconButton(
                    icon: const Icon(Icons.delete, color: Colors.white),
                    onPressed: _clearLogs,
                    tooltip: 'Clear logs',
                  ),
                ],
              ),
            ),
          ),

          // Expanded content
          if (_isExpanded) ...[
            // Status row
            Container(
              padding: const EdgeInsets.all(8),
              color: Colors.grey[850],
              child: Row(
                children: [
                  _InfoChip(
                    label: 'Token',
                    value: sessionToken != null
                        ? '${sessionToken.substring(0, 8)}...'
                        : 'None',
                  ),
                  const SizedBox(width: 8),
                  const _InfoChip(
                    label: 'gRPC',
                    value: '127.0.0.1:50051',
                  ),
                ],
              ),
            ),

            // Logs
            Container(
              height: 200,
              color: Colors.black,
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(8),
                child: SelectableText(
                  _logs.isEmpty ? 'No logs yet' : _logs,
                  style: const TextStyle(
                    color: Colors.greenAccent,
                    fontFamily: 'monospace',
                    fontSize: 10,
                  ),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _StatusIndicator extends StatelessWidget {
  final String label;
  final String value;
  final Color color;

  const _StatusIndicator({
    required this.label,
    required this.value,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          '$label: ',
          style: const TextStyle(color: Colors.grey, fontSize: 12),
        ),
        Container(
          width: 8,
          height: 8,
          decoration: BoxDecoration(
            color: color,
            shape: BoxShape.circle,
          ),
        ),
        const SizedBox(width: 4),
        Text(
          value,
          style: TextStyle(color: color, fontSize: 12),
        ),
      ],
    );
  }
}

class _InfoChip extends StatelessWidget {
  final String label;
  final String value;

  const _InfoChip({
    required this.label,
    required this.value,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: Colors.grey[800],
        borderRadius: BorderRadius.circular(4),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            '$label: ',
            style: const TextStyle(
              color: Colors.grey,
              fontSize: 11,
            ),
          ),
          Text(
            value,
            style: const TextStyle(
              color: Colors.white,
              fontSize: 11,
              fontWeight: FontWeight.bold,
            ),
          ),
        ],
      ),
    );
  }
}
