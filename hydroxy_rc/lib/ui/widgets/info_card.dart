import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/extensions.dart';

/// Rounded, bordered container used to display a single record (tracker, peer,
/// file, ...) in compact / mobile layouts instead of a wide data table row.
class InfoCard extends StatelessWidget {
  final Widget child;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final EdgeInsetsGeometry padding;
  final EdgeInsetsGeometry margin;
  final Color? accentColor;

  const InfoCard({
    super.key,
    required this.child,
    this.onTap,
    this.onLongPress,
    this.padding = const EdgeInsets.all(12),
    this.margin = const EdgeInsets.only(bottom: 8),
    this.accentColor,
  });

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    final radius = BorderRadius.circular(10);

    return Padding(
      padding: margin,
      child: Material(
        color: colors.dataCardBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: radius,
          side: BorderSide(color: colors.dataCardBorderColor, width: 0.8),
        ),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: onTap,
          onLongPress: onLongPress,
          child: Container(
            decoration: accentColor == null
                ? null
                : BoxDecoration(
                    border: Border(left: BorderSide(color: accentColor!, width: 3)),
                  ),
            padding: padding,
            child: child,
          ),
        ),
      ),
    );
  }
}

/// Small label / value pair laid out vertically, used inside [InfoCard] grids.
class InfoField extends StatelessWidget {
  final String label;
  final String value;
  final Color? valueColor;
  final IconData? icon;

  const InfoField({
    super.key,
    required this.label,
    required this.value,
    this.valueColor,
    this.icon,
  });

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(label, style: TextStyle(color: colors.dataLabelTextColor, fontSize: 10)),
        const SizedBox(height: 2),
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              Icon(icon, size: 12, color: valueColor ?? colors.tableSecondaryTextColor),
              const SizedBox(width: 3),
            ],
            Flexible(
              child: Text(
                value,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: valueColor ?? colors.tablePrimaryTextColor,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

/// Lays out [InfoField]s in rows of [columns] equally-sized cells.
class InfoFieldGrid extends StatelessWidget {
  final List<Widget> fields;
  final int columns;
  final double rowSpacing;

  const InfoFieldGrid({
    super.key,
    required this.fields,
    this.columns = 3,
    this.rowSpacing = 8,
  });

  @override
  Widget build(BuildContext context) {
    final rows = <Widget>[];
    for (var i = 0; i < fields.length; i += columns) {
      final cells = <Widget>[];
      for (var j = 0; j < columns; j++) {
        final idx = i + j;
        cells.add(Expanded(child: idx < fields.length ? fields[idx] : const SizedBox.shrink()));
      }
      if (rows.isNotEmpty) rows.add(SizedBox(height: rowSpacing));
      rows.add(Row(crossAxisAlignment: CrossAxisAlignment.start, children: cells));
    }
    return Column(mainAxisSize: MainAxisSize.min, children: rows);
  }
}
