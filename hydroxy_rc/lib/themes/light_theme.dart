import 'package:flutter/material.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';

const String lightThemeCode = 'light';

class LightTheme extends AppTheme {
  @override
  String get themeName => 'Light';
  @override
  String get themeCode => lightThemeCode;
  @override
  Brightness get brightness => Brightness.light;

  // App Shell & Layout
  @override
  Color get scaffoldBackgroundColor => const Color(0xFFF3F4F6);
  @override
  Color get titleBarBackgroundColor => const Color(0xFFFFFFFF);
  @override
  Color get titleBarBorderColor => const Color(0xFFE5E7EB);
  @override
  Color get titleBarTextColor => const Color(0xFF111827);
  @override
  Color get titleBarIconColor => const Color(0xFF4B5563);
  @override
  Color get titleBarLogoColor => const Color(0xFF059669);

  @override
  Color get statusBarBackgroundColor => const Color(0xFFFFFFFF);
  @override
  Color get statusBarBorderColor => const Color(0xFFE5E7EB);
  @override
  Color get statusBarTextColor => const Color(0xFF4B5563);
  @override
  Color get statusBarIconColor => const Color(0xFF9CA3AF);

  @override
  Color get scrollbarThumbColor => const Color(0xFF9CA3AF);
  @override
  Color get scrollbarTrackColor => const Color(0xFFE5E7EB);
  @override
  Color get dividerColor => const Color(0xFFE5E7EB);

  // Sidebar & Categories Tree
  @override
  Color get sidebarBackgroundColor => const Color(0xFFFFFFFF);
  @override
  Color get sidebarBorderColor => const Color(0xFFE5E7EB);
  @override
  Color get sidebarHeaderTextColor => const Color(0xFF6B7280);
  @override
  Color get sidebarItemTextColor => const Color(0xFF374151);
  @override
  Color get sidebarSelectedItemTextColor => const Color(0xFF059669);
  @override
  Color get sidebarSelectedItemBackgroundColor => const Color(0xFFECFDF5);
  @override
  Color get sidebarItemHoverColor => const Color(0xFFF9FAFB);
  @override
  Color get sidebarIconColor => const Color(0xFF9CA3AF);
  @override
  Color get sidebarSelectedIconColor => const Color(0xFF059669);
  @override
  Color get sidebarBadgeBackgroundColor => const Color(0xFFF3F4F6);
  @override
  Color get sidebarSelectedBadgeBackgroundColor => const Color(0xFF059669).withOpacity(0.15);
  @override
  Color get sidebarBadgeTextColor => const Color(0xFF6B7280);
  @override
  Color get sidebarSelectedBadgeTextColor => const Color(0xFF059669);
  @override
  Color get sidebarTreeLineColor => const Color(0xFFD1D5DB);

  // Search Bar
  @override
  Color get searchBarBackgroundColor => const Color(0xFFF9FAFB);
  @override
  Color get searchBarBorderColor => const Color(0xFFD1D5DB);
  @override
  Color get searchBarErrorBorderColor => const Color(0xFFDC2626);
  @override
  Color get searchBarActiveBorderColor => const Color(0xFF059669);
  @override
  Color get searchBarTextColor => const Color(0xFF111827);
  @override
  Color get searchBarHintColor => const Color(0xFF9CA3AF);
  @override
  Color get searchBarIconColor => const Color(0xFF6B7280);
  @override
  Color get searchBarRegexActiveBackgroundColor => const Color(0xFF059669).withOpacity(0.15);
  @override
  Color get searchBarRegexActiveTextColor => const Color(0xFF059669);
  @override
  Color get searchBarRegexInactiveTextColor => const Color(0xFF9CA3AF);

  // Torrent Data Table
  @override
  Color get tableHeaderBackgroundColor => const Color(0xFFF9FAFB);
  @override
  Color get tableHeaderTextColor => const Color(0xFF4B5563);
  @override
  Color get tableRowBackgroundColor => const Color(0xFFFFFFFF);
  @override
  Color get tableAlternatingRowBackgroundColor => const Color(0xFFF9FAFB);
  @override
  Color get tableRowHoverColor => const Color(0xFFF3F4F6);
  @override
  Color get tableRowSelectedColor => const Color(0xFFE0E7FF);
  @override
  Color get tableDividerColor => const Color(0xFFE5E7EB);
  @override
  Color get tablePrimaryTextColor => const Color(0xFF111827);
  @override
  Color get tableSecondaryTextColor => const Color(0xFF6B7280);
  @override
  Color get tableAccentTextColor => const Color(0xFF059669);
  @override
  Color get tableProgressBarTrackColor => const Color(0xFFE5E7EB);
  @override
  Color get tableProgressBarProgressColor => const Color(0xFF059669);

  // Details Panel & Tabs
  @override
  Color get detailsPanelBackgroundColor => const Color(0xFFFFFFFF);
  @override
  Color get detailsPanelBorderColor => const Color(0xFFE5E7EB);
  @override
  Color get detailsPanelResizeHandleColor => const Color(0xFFD1D5DB);
  @override
  Color get tabBarBackgroundColor => const Color(0xFFF9FAFB);
  @override
  Color get tabBarActiveIndicatorColor => const Color(0xFF059669);
  @override
  Color get tabBarActiveTextColor => const Color(0xFF111827);
  @override
  Color get tabBarInactiveTextColor => const Color(0xFF6B7280);
  @override
  Color get tabBarActiveIconColor => const Color(0xFF059669);
  @override
  Color get tabBarInactiveIconColor => const Color(0xFF9CA3AF);
  @override
  Color get detailsContentBackgroundColor => const Color(0xFFFFFFFF);
  @override
  Color get detailsEmptyTextColor => const Color(0xFF9CA3AF);

  // Piece Map
  @override
  Color get pieceMapBackgroundColor => const Color(0xFFE5E7EB);
  @override
  Color get pieceMapCompletedPieceColor => const Color(0xFF059669);
  @override
  Color get pieceMapBorderColor => const Color(0xFFD1D5DB);

  // Data Cards & Status Badges
  @override
  Color get dataCardBackgroundColor => const Color(0xFFF9FAFB);
  @override
  Color get dataCardBorderColor => const Color(0xFFE5E7EB);
  @override
  Color get dataLabelTextColor => const Color(0xFF6B7280);
  @override
  Color get dataValueTextColor => const Color(0xFF111827);

  @override
  Color get badgeSuccessBackgroundColor => const Color(0xFFDEF7EC);
  @override
  Color get badgeSuccessTextColor => const Color(0xFF03543F);
  @override
  Color get badgeWarningBackgroundColor => const Color(0xFFFEF08A);
  @override
  Color get badgeWarningTextColor => const Color(0xFF713F12);
  @override
  Color get badgeErrorBackgroundColor => const Color(0xFFFDE8E8);
  @override
  Color get badgeErrorTextColor => const Color(0xFF9B1C1C);
  @override
  Color get badgeInfoBackgroundColor => const Color(0xFFE1EFFE);
  @override
  Color get badgeInfoTextColor => const Color(0xFF1E429F);

  // Context Menus & Tooltips & Snacks
  @override
  Color get popupMenuBackgroundColor => const Color(0xFFFFFFFF);
  @override
  Color get popupMenuBorderColor => const Color(0xFFE5E7EB);
  @override
  Color get popupMenuTextColor => const Color(0xFF111827);
  @override
  Color get popupMenuDividerColor => const Color(0xFFE5E7EB);
  @override
  Color get popupMenuHoverColor => const Color(0xFFF3F4F6);
  @override
  Color get snackBarBackgroundColor => const Color(0xFF1F2937);
  @override
  Color get snackBarTextColor => const Color(0xFFF9FAFB);
  @override
  Color get tooltipBackgroundColor => const Color(0xFF1F2937);
  @override
  Color get tooltipTextColor => const Color(0xFFF9FAFB);

  // Auth / Login Screen
  @override
  Color get authCardBackgroundColor => const Color(0xFFFFFFFF);
  @override
  Color get authCardBorderColor => const Color(0xFFE5E7EB);
  @override
  Color get authTitleTextColor => const Color(0xFF111827);
  @override
  Color get authSubtitleTextColor => const Color(0xFF6B7280);
  @override
  Color get authInputBackgroundColor => const Color(0xFFF9FAFB);
  @override
  Color get authInputBorderColor => const Color(0xFFD1D5DB);
  @override
  Color get authInputActiveBorderColor => const Color(0xFF059669);
  @override
  Color get authInputTextColor => const Color(0xFF111827);
  @override
  Color get authInputHintColor => const Color(0xFF9CA3AF);
  @override
  Color get authButtonBackgroundColor => const Color(0xFF059669);
  @override
  Color get authButtonTextColor => const Color(0xFFFFFFFF);
  @override
  Color get authButtonHoverColor => const Color(0xFF047857);

  // Metrics, Transfer & Accent Colors
  @override
  Color get downloadSpeedColor => const Color(0xFF059669);
  @override
  Color get uploadSpeedColor => const Color(0xFF2563EB);
  @override
  Color get memoryUsageColor => const Color(0xFFD97706);
  @override
  Color get accentGreenColor => const Color(0xFF059669);
  @override
  Color get accentAmberColor => const Color(0xFFD97706);
  @override
  Color get accentRedColor => const Color(0xFFDC2626);
  @override
  Color get accentBlueColor => const Color(0xFF2563EB);
  @override
  Color get accentPurpleColor => const Color(0xFF7C3AED);
}
