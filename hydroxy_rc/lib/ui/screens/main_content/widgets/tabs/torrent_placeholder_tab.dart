import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/models.dart';

class TorrentPlaceholderTab extends StatelessWidget {
  final String title;
  final IconData icon;
  final TorrentStructure torrent;
  final List<String>? mockHeaders;

  const TorrentPlaceholderTab({
    super.key,
    required this.title,
    required this.icon,
    required this.torrent,
    this.mockHeaders,
  });

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;

    if (mockHeaders != null && mockHeaders!.isNotEmpty) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            height: 24,
            color: colors.tableHeaderBackgroundColor,
            padding: const EdgeInsets.symmetric(horizontal: 8),
            child: Row(
              children: mockHeaders!.map((header) {
                return Expanded(
                  child: Text(
                    header,
                    style: TextStyle(
                      color: colors.tableHeaderTextColor,
                      fontSize: 11,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                );
              }).toList(),
            ),
          ),
          Divider(color: colors.dividerColor, height: 1),
          Expanded(
            child: Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(icon, size: 28, color: Color.fromRGBO(colors.tableSecondaryTextColor.red, colors.tableSecondaryTextColor.green, colors.tableSecondaryTextColor.blue, 0.4)),
                  const SizedBox(height: 6),
                  Text(
                    'No $title data available for ${torrent.name}',
                    style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 11),
                  ),
                ],
              ),
            ),
          ),
        ],
      );
    }

    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 32, color: Color.fromRGBO(colors.tableSecondaryTextColor.red, colors.tableSecondaryTextColor.green, colors.tableSecondaryTextColor.blue, 0.4)),
          const SizedBox(height: 8),
          Text(
            '$title information for "${torrent.name}"',
            style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 12),
          ),
        ],
      ),
    );
  }
}
