part of 'themes_bloc.dart';

abstract class ThemesEvent {
  const ThemesEvent();
}

class ChangeThemeEvent extends ThemesEvent {
  final AppTheme appTheme;
  const ChangeThemeEvent(this.appTheme);
}

class ChangeThemeByCodeEvent extends ThemesEvent {
  final String themeCode;
  const ChangeThemeByCodeEvent(this.themeCode);
}
