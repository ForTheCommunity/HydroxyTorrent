import 'package:bloc/bloc.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';
import 'package:hydroxy_rc/themes/dark_theme.dart';
import 'package:hydroxy_rc/themes/dracula_theme.dart';
import 'package:hydroxy_rc/themes/light_theme.dart';
import 'package:hydroxy_rc/themes/matrix_theme.dart';
import 'package:hydroxy_rc/themes/nord_theme.dart';

part 'themes_event.dart';
part 'themes_state.dart';

class ThemesBloc extends Bloc<ThemesEvent, ThemesState> {
  static final List<AppTheme> availableThemes = [
    DarkTheme(),
    MatrixTheme(),
    NordTheme(),
    DraculaTheme(),
    LightTheme(),
  ];

  static AppTheme themeForCode(String code) {
    for (final theme in availableThemes) {
      if (theme.themeCode == code) return theme;
    }
    return DarkTheme();
  }

  ThemesBloc() : super(ThemesState.fromTheme(DarkTheme())) {
    on<ChangeThemeEvent>((event, emit) {
      emit(ThemesState.fromTheme(event.appTheme));
    });

    on<ChangeThemeByCodeEvent>((event, emit) {
      final theme = themeForCode(event.themeCode);
      emit(ThemesState.fromTheme(theme));
    });
  }
}
