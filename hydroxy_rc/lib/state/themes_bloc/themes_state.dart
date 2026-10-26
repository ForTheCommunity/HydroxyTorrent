part of 'themes_bloc.dart';

class ThemesState {
  final AppTheme currentAppTheme;
  final String currentAppThemeName;
  final String currentAppThemeCode;

  const ThemesState({
    required this.currentAppTheme,
    required this.currentAppThemeName,
    required this.currentAppThemeCode,
  });

  factory ThemesState.fromTheme(AppTheme theme) {
    return ThemesState(
      currentAppTheme: theme,
      currentAppThemeName: theme.themeName,
      currentAppThemeCode: theme.themeCode,
    );
  }
}
