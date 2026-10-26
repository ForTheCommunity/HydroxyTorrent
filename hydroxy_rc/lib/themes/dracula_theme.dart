import 'package:flutter/material.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';

const String draculaThemeCode = 'dracula';

class DraculaTheme extends AppTheme {
  @override
  String get themeName => 'Dracula';
  @override
  String get themeCode => draculaThemeCode;
  @override
  Brightness get brightness => Brightness.dark;

  // App Shell & Layout
  @override
  Color get scaffoldBackgroundColor => const Color(0xFF282A36);
  @override
  Color get titleBarBackgroundColor => const Color(0xFF21222C);
  @override
  Color get titleBarBorderColor => const Color(0xFF44475A);
  @override
  Color get titleBarTextColor => const Color(0xFFF8F8F2);
  @override
  Color get titleBarIconColor => const Color(0xFFBD93F9);
  @override
  Color get titleBarLogoColor => const Color(0xFF50FA7B);

  @override
  Color get statusBarBackgroundColor => const Color(0xFF21222C);
  @override
  Color get statusBarBorderColor => const Color(0xFF44475A);
  @override
  Color get statusBarTextColor => const Color(0xFFF8F8F2);
  @override
  Color get statusBarIconColor => const Color(0xFF6272A4);

  @override
  Color get scrollbarThumbColor => const Color(0xFFBD93F9);
  @override
  Color get scrollbarTrackColor => const Color(0xFF343746);
  @override
  Color get dividerColor => const Color(0xFF44475A);

  // Sidebar & Categories Tree
  @override
  Color get sidebarBackgroundColor => const Color(0xFF21222C);
  @override
  Color get sidebarBorderColor => const Color(0xFF44475A);
  @override
  Color get sidebarHeaderTextColor => const Color(0xFF6272A4);
  @override
  Color get sidebarItemTextColor => const Color(0xFFF8F8F2);
  @override
  Color get sidebarSelectedItemTextColor => const Color(0xFF50FA7B);
  @override
  Color get sidebarSelectedItemBackgroundColor => const Color(0xFF44475A);
  @override
  Color get sidebarItemHoverColor => const Color(0xFF343746);
  @override
  Color get sidebarIconColor => const Color(0xFF8BE9FD);
  @override
  Color get sidebarSelectedIconColor => const Color(0xFF50FA7B);
  @override
  Color get sidebarBadgeBackgroundColor => const Color(0xFF44475A);
  @override
  Color get sidebarSelectedBadgeBackgroundColor => const Color.fromRGBO(80, 250, 123, 0.25);
  @override
  Color get sidebarBadgeTextColor => const Color(0xFFF8F8F2);
  @override
  Color get sidebarSelectedBadgeTextColor => const Color(0xFF50FA7B);
  @override
  Color get sidebarTreeLineColor => const Color.fromRGBO(98, 114, 164, 0.4);

  // Search Bar
  @override
  Color get searchBarBackgroundColor => const Color(0xFF21222C);
  @override
  Color get searchBarBorderColor => const Color(0xFF44475A);
  @override
  Color get searchBarErrorBorderColor => const Color(0xFFFF5555);
  @override
  Color get searchBarActiveBorderColor => const Color(0xFFBD93F9);
  @override
  Color get searchBarTextColor => const Color(0xFFF8F8F2);
  @override
  Color get searchBarHintColor => const Color(0xFF6272A4);
  @override
  Color get searchBarIconColor => const Color(0xFFBD93F9);
  @override
  Color get searchBarRegexActiveBackgroundColor => const Color.fromRGBO(189, 147, 249, 0.25);
  @override
  Color get searchBarRegexActiveTextColor => const Color(0xFFBD93F9);
  @override
  Color get searchBarRegexInactiveTextColor => const Color(0xFF6272A4);

  // Torrent Data Table
  @override
  Color get tableHeaderBackgroundColor => const Color(0xFF21222C);
  @override
  Color get tableHeaderTextColor => const Color(0xFFF8F8F2);
  @override
  Color get tableRowBackgroundColor => const Color(0xFF282A36);
  @override
  Color get tableAlternatingRowBackgroundColor => const Color(0xFF2E303E);
  @override
  Color get tableRowHoverColor => const Color(0xFF383A49);
  @override
  Color get tableRowSelectedColor => const Color(0xFF44475A);
  @override
  Color get tableDividerColor => const Color(0xFF4D5066);
  @override
  Color get tableColumnSeparatorColor => const Color(0xFF6272A4);
  @override
  Color get tablePrimaryTextColor => const Color(0xFFF8F8F2);
  @override
  Color get tableSecondaryTextColor => const Color(0xFF6272A4);
  @override
  Color get tableAccentTextColor => const Color(0xFF50FA7B);
  @override
  Color get tableProgressBarTrackColor => const Color(0xFF44475A);
  @override
  Color get tableProgressBarProgressColor => const Color(0xFF50FA7B);

  // Details Panel & Tabs
  @override
  Color get detailsPanelBackgroundColor => const Color(0xFF21222C);
  @override
  Color get detailsPanelBorderColor => const Color(0xFF44475A);
  @override
  Color get detailsPanelResizeHandleColor => const Color(0xFF6272A4);
  @override
  Color get tabBarBackgroundColor => const Color(0xFF21222C);
  @override
  Color get tabBarActiveIndicatorColor => const Color(0xFFBD93F9);
  @override
  Color get tabBarActiveTextColor => const Color(0xFFF8F8F2);
  @override
  Color get tabBarInactiveTextColor => const Color(0xFF6272A4);
  @override
  Color get tabBarActiveIconColor => const Color(0xFFBD93F9);
  @override
  Color get tabBarInactiveIconColor => const Color(0xFF6272A4);
  @override
  Color get detailsContentBackgroundColor => const Color(0xFF1E1F29);
  @override
  Color get detailsEmptyTextColor => const Color(0xFF6272A4);

  // Piece Map
  @override
  Color get pieceMapBackgroundColor => const Color(0xFF21222C);
  @override
  Color get pieceMapCompletedPieceColor => const Color(0xFF50FA7B);
  @override
  Color get pieceMapBorderColor => const Color(0xFF44475A);

  // Data Cards & Status Badges
  @override
  Color get dataCardBackgroundColor => const Color(0xFF282A36);
  @override
  Color get dataCardBorderColor => const Color(0xFF44475A);
  @override
  Color get dataLabelTextColor => const Color(0xFF6272A4);
  @override
  Color get dataValueTextColor => const Color(0xFFF8F8F2);

  @override
  Color get badgeSuccessBackgroundColor => const Color.fromRGBO(80, 250, 123, 0.2);
  @override
  Color get badgeSuccessTextColor => const Color(0xFF50FA7B);
  @override
  Color get badgeWarningBackgroundColor => const Color.fromRGBO(241, 250, 140, 0.2);
  @override
  Color get badgeWarningTextColor => const Color(0xFFF1FA8C);
  @override
  Color get badgeErrorBackgroundColor => const Color.fromRGBO(255, 85, 85, 0.2);
  @override
  Color get badgeErrorTextColor => const Color(0xFFFF5555);
  @override
  Color get badgeInfoBackgroundColor => const Color.fromRGBO(139, 233, 253, 0.2);
  @override
  Color get badgeInfoTextColor => const Color(0xFF8BE9FD);

  // Context Menus & Tooltips & Snacks
  @override
  Color get popupMenuBackgroundColor => const Color(0xFF282A36);
  @override
  Color get popupMenuBorderColor => const Color(0xFF44475A);
  @override
  Color get popupMenuTextColor => const Color(0xFFF8F8F2);
  @override
  Color get popupMenuDividerColor => const Color(0xFF44475A);
  @override
  Color get popupMenuHoverColor => const Color(0xFF44475A);
  @override
  Color get snackBarBackgroundColor => const Color(0xFF21222C);
  @override
  Color get snackBarTextColor => const Color(0xFFF8F8F2);
  @override
  Color get tooltipBackgroundColor => const Color(0xFF282A36);
  @override
  Color get tooltipTextColor => const Color(0xFFF8F8F2);

  // Auth / Login Screen
  @override
  Color get authCardBackgroundColor => const Color(0xFF282A36);
  @override
  Color get authCardBorderColor => const Color(0xFF44475A);
  @override
  Color get authTitleTextColor => const Color(0xFFF8F8F2);
  @override
  Color get authSubtitleTextColor => const Color(0xFF6272A4);
  @override
  Color get authInputBackgroundColor => const Color(0xFF21222C);
  @override
  Color get authInputBorderColor => const Color(0xFF44475A);
  @override
  Color get authInputActiveBorderColor => const Color(0xFFBD93F9);
  @override
  Color get authInputTextColor => const Color(0xFFF8F8F2);
  @override
  Color get authInputHintColor => const Color(0xFF6272A4);
  @override
  Color get authButtonBackgroundColor => const Color(0xFFBD93F9);
  @override
  Color get authButtonTextColor => const Color(0xFF282A36);
  @override
  Color get authButtonHoverColor => const Color(0xFFFF79C6);

  // Metrics, Transfer & Accent Colors
  @override
  Color get downloadSpeedColor => const Color(0xFF50FA7B);
  @override
  Color get uploadSpeedColor => const Color(0xFF8BE9FD);
  @override
  Color get memoryUsageColor => const Color(0xFFFFB86C);
  @override
  Color get accentGreenColor => const Color(0xFF50FA7B);
  @override
  Color get accentAmberColor => const Color(0xFFF1FA8C);
  @override
  Color get accentRedColor => const Color(0xFFFF5555);
  @override
  Color get accentBlueColor => const Color(0xFF8BE9FD);
  @override
  Color get accentPurpleColor => const Color(0xFFBD93F9);
}
